use std::io;

use crate::notification::AdapterSelection;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Command {
    Run,
    NotifyTest(AdapterSelection),
}

pub(super) fn parse(arguments: impl IntoIterator<Item = String>) -> Result<Command, io::Error> {
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    match arguments.as_slice() {
        [] => Ok(Command::Run),
        [notify, test] if notify == "notify" && test == "test" => {
            Ok(Command::NotifyTest(AdapterSelection::All))
        }
        [notify, test, adapter] if notify == "notify" && test == "test" => {
            Ok(Command::NotifyTest(AdapterSelection::parse(adapter)?))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: lirvena [notify test [all|bark|webhook|smtp]]",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, parse};
    use crate::notification::AdapterSelection;

    #[test]
    fn notification_test_command_is_closed() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            parse([String::from("notify"), String::from("test")])?,
            Command::NotifyTest(AdapterSelection::All)
        );
        assert_eq!(
            parse([
                String::from("notify"),
                String::from("test"),
                String::from("bark")
            ])?,
            Command::NotifyTest(AdapterSelection::Bark)
        );
        assert!(
            parse([
                String::from("notify"),
                String::from("test"),
                String::from("unknown")
            ])
            .is_err()
        );
        Ok(())
    }
}
