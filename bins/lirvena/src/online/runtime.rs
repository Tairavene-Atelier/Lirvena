use std::collections::BTreeMap;
use std::io;
use std::path::Path;
use std::time::Duration;

use account_api::{
    AccountActionReceiver, AccountEvent, AccountEventPublisher, AccountIdentity, InboundMessage,
};
use account_runtime::AssignedRealm;
use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_directory::FriendEntry;
use qq_domain::{OnlineAction, OnlineDirective, OnlineGeneration, OnlineMachine, OnlineState};
use qq_login::{CredentialLogin, QrDevice};
use qq_profile::{LinuxNtProfile, decode_online_plan};
use qq_session::AuthenticatedSession;
use tokio::net::TcpStream;
use tokio::time::sleep;

use super::actions::{ActionResources, execute_account_action};
use super::media::MediaRuntime;
use super::message_registry::MessageRegistry;
use super::notices;
use super::packets::{PacketContext, PacketRuntime};
use super::push::{DecodedPush, PushRuntime};
use crate::action_runtime::{self, BootstrapContext};
use crate::support::now_ms;

const FIRST_GENERATION: u64 = 1;

pub(crate) struct OnlineContext<'a> {
    pub ceylith: &'a InstallationClient,
    pub qq: &'a mut AuthenticatedSession<TcpStream>,
    pub profile: &'a LinuxNtProfile,
    pub realm: AssignedRealm,
    pub device: &'a QrDevice,
    pub credential: &'a CredentialLogin,
    pub uin: u64,
    pub account_slot_id: AccountSlotId,
}

pub(crate) struct OnlineRuntime {
    machine: OnlineMachine,
    packets: PacketRuntime,
    pushes: PushRuntime,
    generation: OnlineGeneration,
    identity: AccountIdentity,
    events: AccountEventPublisher,
    friends: BTreeMap<u32, FriendEntry>,
    messages: MessageRegistry,
    media: MediaRuntime,
}

impl OnlineRuntime {
    pub(crate) fn new(
        profile: &LinuxNtProfile,
        device: &QrDevice,
        identity: AccountIdentity,
        events: AccountEventPublisher,
        state_directory: &Path,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let messages = MessageRegistry::open(state_directory, identity.local_id())?;
        Ok(Self {
            machine: OnlineMachine::new(decode_online_plan(profile)?),
            packets: PacketRuntime::new(profile, device)?,
            pushes: PushRuntime::new(profile)?,
            generation: OnlineGeneration::new(FIRST_GENERATION)?,
            identity,
            events,
            friends: BTreeMap::new(),
            messages,
            media: MediaRuntime::new(state_directory)?,
        })
    }

    pub(crate) async fn bootstrap(
        &mut self,
        mut context: OnlineContext<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        require_action(
            self.machine.start(self.generation)?,
            self.generation,
            ActionClass::InitialSync,
        )?;
        let outcome = self
            .packets
            .synchronize(
                PacketContext {
                    qq: context.qq,
                    push_plan: self.pushes.plan(),
                    profile: context.profile,
                    credential: context.credential,
                    uin: context.uin,
                },
                false,
            )
            .await?;
        require_action(
            self.machine.initial_sync_succeeded(
                self.generation,
                now_ms()?,
                outcome.delayed_after_ms,
            )?,
            self.generation,
            ActionClass::SecurityBootstrap,
        )?;
        if requires_full_bootstrap(context.realm) {
            action_runtime::run(BootstrapContext {
                ceylith: context.ceylith,
                qq: context.qq,
                push_plan: self.pushes.plan(),
                profile: context.profile,
                device: context.device,
                credential: context.credential,
                uin: context.uin,
                account_slot_id: context.account_slot_id,
            })
            .await?;
        }
        match self
            .machine
            .security_bootstrap_succeeded(self.generation, now_ms()?)?
        {
            OnlineDirective::Dispatch {
                generation,
                action: OnlineAction::StatusConfirmation(_),
            } if generation == self.generation && self.packets.has_status_confirmation() => {
                self.packets
                    .confirm_status(PacketContext {
                        qq: context.qq,
                        push_plan: self.pushes.plan(),
                        profile: context.profile,
                        credential: context.credential,
                        uin: context.uin,
                    })
                    .await?;
                require_entered(
                    self.machine
                        .status_confirmation_completed(self.generation, now_ms()?)?,
                    self.generation,
                )?;
            }
            OnlineDirective::EnteredOnline(generation)
                if generation == self.generation && !self.packets.has_status_confirmation() => {}
            _ => return Err(io::Error::other("Profile online gates are inconsistent").into()),
        }
        if self.machine.state() != OnlineState::Online(self.generation) {
            return Err(io::Error::other("online generation did not enter Online").into());
        }
        self.pushes
            .drain(
                context.qq,
                &mut self.packets,
                context.profile,
                context.credential,
                context.uin,
            )
            .await?;
        self.publish_pending_events(&mut context).await?;
        Ok(())
    }

    pub(crate) async fn run(
        &mut self,
        mut context: OnlineContext<'_>,
        mut actions: AccountActionReceiver,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = crate::qq::session_key(context.credential)?;
        let auth = crate::qq::authenticated(context.uin, context.credential, &key)?;
        loop {
            self.pushes
                .drain(
                    context.qq,
                    &mut self.packets,
                    context.profile,
                    context.credential,
                    context.uin,
                )
                .await?;
            self.publish_pending_events(&mut context).await?;
            let due_at = self
                .machine
                .next_due_ms()
                .ok_or_else(|| io::Error::other("online generation has no continuity schedule"))?;
            let delay = Duration::from_millis(due_at.saturating_sub(now_ms()?).max(1));
            tokio::select! {
                result = tokio::signal::ctrl_c() => {
                    result?;
                    self.machine.stop();
                    return Ok(());
                }
                inbound = context.qq.read_push(&auth, |route| self.pushes.admits(route)) => {
                    match inbound {
                        Ok(push) => self.pushes.handle(
                            context.qq,
                            &mut self.packets,
                            context.profile,
                            context.credential,
                            context.uin,
                            push,
                        ).await?,
                        Err(error) if error.is_idle_timeout() => continue,
                        Err(error) => return Err(error.into()),
                    }
                    self.publish_pending_events(&mut context).await?;
                    continue;
                }
                pending = actions.receive() => {
                    let Some(pending) = pending else {
                        return Err(io::Error::other("account action channel ended").into());
                    };
                    let result = execute_account_action(
                        pending.request(),
                        &self.identity,
                        &self.packets,
                        &self.pushes,
                        &mut self.friends,
                        &mut ActionResources {
                            messages: &mut self.messages,
                            media: &mut self.media,
                        },
                        &mut context,
                    ).await;
                    pending.complete(result);
                    continue;
                }
                () = sleep(delay) => {}
            }
            if let Err(error) = self
                .execute_due(context.qq, context.profile, context.credential, context.uin)
                .await
            {
                let _directive = self.machine.required_action_failed(self.generation);
                return Err(error);
            }
        }
    }

    async fn publish_pending_events(
        &mut self,
        context: &mut OnlineContext<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        while let Some(event) = self.pushes.pop_event() {
            match event {
                DecodedPush::Message(message) => self.publish_message(*message)?,
                DecodedPush::GroupNotice {
                    notice,
                    occurred_at,
                    ..
                } => {
                    match notices::resolve_group_notice(
                        &self.identity,
                        &self.packets,
                        &self.pushes,
                        notice,
                        occurred_at,
                        context,
                    )
                    .await
                    {
                        Some(notice) => {
                            let _delivered = self
                                .events
                                .publish(AccountEvent::GroupNotice(Box::new(notice)));
                        }
                        None => eprintln!(
                            "Lirvena retained no OneBot group notice because its authenticated UID \
                             could not be resolved without inventing an account identifier"
                        ),
                    }
                }
                DecodedPush::GroupRequest {
                    signal,
                    occurred_at,
                    ..
                } => match super::requests::resolve_group_request(
                    &self.identity,
                    &self.packets,
                    &self.pushes,
                    signal,
                    occurred_at,
                    context,
                )
                .await
                {
                    Ok(Some(request)) => {
                        let _delivered = self
                            .events
                            .publish(AccountEvent::GroupRequest(Box::new(request)));
                    }
                    Ok(None) | Err(_) => eprintln!(
                        "Lirvena retained no OneBot group request because QQ's authenticated \
                        request reference could not be resolved uniquely"
                    ),
                },
                DecodedPush::FriendRequest { signal, .. } => {
                    match super::requests::resolve_friend_request(
                        &self.identity,
                        &self.packets,
                        &self.pushes,
                        signal,
                        context,
                    )
                    .await
                    {
                        Ok(Some(request)) => {
                            let _delivered = self
                                .events
                                .publish(AccountEvent::FriendRequest(Box::new(request)));
                        }
                        Ok(None) | Err(_) => eprintln!(
                            "Lirvena retained no OneBot friend request because QQ's authenticated \
                         request reference could not be resolved uniquely"
                        ),
                    }
                }
            }
        }
        Ok(())
    }

    fn publish_message(
        &mut self,
        message: super::push::DecodedMessage,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let segment_count = message.rich_text().map_or(0, |body| body.elements().len());
        let (envelope, rich_text) = message.into_parts();
        let message_class = envelope.class();
        let (message_id, recall) = self.messages.prepare_inbound(&envelope)?;
        let reply_ids = self
            .messages
            .resolve_reply_ids(&envelope, rich_text.as_ref())?;
        let message = InboundMessage::new(self.identity.clone(), message_id, envelope, rich_text)
            .with_reply_ids(reply_ids)?;
        self.messages.retain_inbound(&message, recall, now_ms()?)?;
        let _delivered = self
            .events
            .publish(AccountEvent::Message(Box::new(message)));
        eprintln!(
            "Lirvena received authenticated QQ {message_class:?} message with \
             {segment_count} decoded segments"
        );
        Ok(())
    }

    async fn execute_due(
        &mut self,
        qq: &mut AuthenticatedSession<TcpStream>,
        profile: &LinuxNtProfile,
        credential: &CredentialLogin,
        uin: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for directive in self.machine.poll_due(now_ms()?).into_iter().flatten() {
            match directive {
                OnlineDirective::Dispatch {
                    generation,
                    action: OnlineAction::BusinessHeartbeat(_),
                } if generation == self.generation => {
                    let outcome = self
                        .packets
                        .heartbeat(PacketContext {
                            qq,
                            push_plan: self.pushes.plan(),
                            profile,
                            credential,
                            uin,
                        })
                        .await?;
                    self.machine.heartbeat_completed(
                        self.generation,
                        now_ms()?,
                        outcome.requested_interval_ms,
                    )?;
                }
                OnlineDirective::Dispatch {
                    generation,
                    action: OnlineAction::DelayedSync(_),
                } if generation == self.generation => {
                    let outcome = self
                        .packets
                        .synchronize(
                            PacketContext {
                                qq,
                                push_plan: self.pushes.plan(),
                                profile,
                                credential,
                                uin,
                            },
                            true,
                        )
                        .await?;
                    self.machine.delayed_sync_completed(
                        self.generation,
                        now_ms()?,
                        outcome.delayed_after_ms,
                    )?;
                }
                _ => {
                    return Err(
                        io::Error::other("online schedule dispatched an invalid action").into(),
                    );
                }
            }
        }
        Ok(())
    }
}

const fn requires_full_bootstrap(realm: AssignedRealm) -> bool {
    matches!(realm, AssignedRealm::Full)
}

#[cfg(test)]
mod tests {
    use super::requires_full_bootstrap;
    use account_runtime::AssignedRealm;

    #[test]
    fn public_never_enters_the_full_action_flow() {
        assert!(!requires_full_bootstrap(AssignedRealm::Public));
        assert!(requires_full_bootstrap(AssignedRealm::Full));
    }
}

#[derive(Clone, Copy)]
enum ActionClass {
    InitialSync,
    SecurityBootstrap,
}

fn require_action(
    directive: OnlineDirective,
    expected_generation: OnlineGeneration,
    expected: ActionClass,
) -> Result<(), io::Error> {
    let valid = matches!(
        (directive, expected),
        (
            OnlineDirective::Dispatch {
                generation,
                action: OnlineAction::InitialSync(_),
            },
            ActionClass::InitialSync,
        ) if generation == expected_generation
    ) || matches!(
        (directive, expected),
        (
            OnlineDirective::Dispatch {
                generation,
                action: OnlineAction::SecurityBootstrap(_),
            },
            ActionClass::SecurityBootstrap,
        ) if generation == expected_generation
    );
    if valid {
        Ok(())
    } else {
        Err(io::Error::other(
            "Profile online action sequence is inconsistent",
        ))
    }
}

fn require_entered(
    directive: OnlineDirective,
    expected_generation: OnlineGeneration,
) -> Result<(), io::Error> {
    match directive {
        OnlineDirective::EnteredOnline(generation) if generation == expected_generation => Ok(()),
        _ => Err(io::Error::other("online generation did not enter Online")),
    }
}
