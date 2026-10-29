//! `project.openFolder` and `project.newFolder` only ask the interface for a folder picker: each
//! queues its `dialog.*` request, takes no parameters, and opens nothing itself.

use serde_json::json;

use crate::Session;

// covers: L1-H4
#[test]
fn open_folder_and_new_folder_request_their_dialogs_and_open_nothing() {
    for (command, kind) in [("project.openFolder", "dialog.projectOpen"), ("project.newFolder", "dialog.projectNew")] {
        let mut s = Session::empty();
        s.execute(command, &json!({})).unwrap_or_else(|e| panic!("{command}: {e}"));
        let kinds: Vec<&str> = s.ui_requests.iter().map(|r| r.kind.as_str()).collect();
        assert_eq!(kinds, [kind], "{command}");
        assert!(s.docs.is_empty(), "{command} opens no file");
    }
}
