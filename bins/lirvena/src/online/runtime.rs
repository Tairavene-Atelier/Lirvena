use std::io;
use std::time::Duration;

use account_api::{AccountEvent, AccountEventPublisher, AccountIdentity, InboundMessage};
use ceylith_client::InstallationClient;
use ceylith_protocol::AccountSlotId;
use qq_domain::{OnlineAction, OnlineDirective, OnlineGeneration, OnlineMachine, OnlineState};
use qq_login::{CredentialLogin, QrDevice};
use qq_profile::{LinuxNtProfile, decode_online_plan};
use qq_session::AuthenticatedSession;
use tokio::net::TcpStream;
use tokio::time::sleep;

use super::packets::{PacketContext, PacketRuntime};
use super::push::PushRuntime;
use crate::action_runtime::{self, BootstrapContext};
use crate::support::now_ms;

const FIRST_GENERATION: u64 = 1;

pub(crate) struct OnlineContext<'a> {
    pub ceylith: &'a InstallationClient,
    pub qq: &'a mut AuthenticatedSession<TcpStream>,
    pub profile: &'a LinuxNtProfile,
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
}

impl OnlineRuntime {
    pub(crate) fn new(
        profile: &LinuxNtProfile,
        device: &QrDevice,
        identity: AccountIdentity,
        events: AccountEventPublisher,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            machine: OnlineMachine::new(decode_online_plan(profile)?),
            packets: PacketRuntime::new(profile, device)?,
            pushes: PushRuntime::new(profile)?,
            generation: OnlineGeneration::new(FIRST_GENERATION)?,
            identity,
            events,
        })
    }

    pub(crate) async fn bootstrap(
        &mut self,
        context: OnlineContext<'_>,
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
        self.publish_pending_messages();
        Ok(())
    }

    pub(crate) async fn run(
        &mut self,
        qq: &mut AuthenticatedSession<TcpStream>,
        profile: &LinuxNtProfile,
        credential: &CredentialLogin,
        uin: u64,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let key = crate::qq::session_key(credential)?;
        let auth = crate::qq::authenticated(uin, credential, &key)?;
        loop {
            self.pushes
                .drain(qq, &mut self.packets, profile, credential, uin)
                .await?;
            self.publish_pending_messages();
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
                inbound = qq.read_push(&auth, |route| self.pushes.admits(route)) => {
                    match inbound {
                        Ok(push) => self.pushes.handle(
                            qq, &mut self.packets, profile, credential, uin, push
                        ).await?,
                        Err(error) if error.is_idle_timeout() => continue,
                        Err(error) => return Err(error.into()),
                    }
                    self.publish_pending_messages();
                    continue;
                }
                () = sleep(delay) => {}
            }
            if let Err(error) = self.execute_due(qq, profile, credential, uin).await {
                let _directive = self.machine.required_action_failed(self.generation);
                return Err(error);
            }
        }
    }

    fn publish_pending_messages(&mut self) {
        while let Some(message) = self.pushes.pop_message() {
            let segment_count = message.rich_text().map_or(0, |body| body.elements().len());
            let (envelope, rich_text) = message.into_parts();
            let message_class = envelope.class();
            let event = AccountEvent::Message(Box::new(InboundMessage::new(
                self.identity.clone(),
                envelope,
                rich_text,
            )));
            let _delivered = self.events.publish(event);
            eprintln!(
                "Lirvena received authenticated QQ {message_class:?} message with \
                 {segment_count} decoded segments"
            );
        }
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
