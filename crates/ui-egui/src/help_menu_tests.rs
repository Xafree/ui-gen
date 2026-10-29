//! UIGEN has no release channel of its own yet: the Help menu must not offer to look for updates
//! from the upstream FigCraft project, which would replace UIGEN with FigCraft.

use crate::toolbar::help_menu_commands;

// covers: U1
#[test]
fn help_menu_offers_no_update_check() {
    let commands = help_menu_commands();
    assert!(commands.contains(&"help.about"), "the other Help entries stay: {commands:?}");
    assert!(commands.contains(&"help.gettingStarted"), "the other Help entries stay: {commands:?}");
    for command in &commands {
        assert!(!command.to_lowercase().contains("update"), "{command} offers an update check: {commands:?}");
    }
}
