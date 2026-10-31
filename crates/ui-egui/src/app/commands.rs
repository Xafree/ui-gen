use super::{
    Done, FigApp, LeftTab, MobilePanel, NodeId, PANEL_DEFAULT, RightTab, Value, comments, compare_tab, dev_compare, dialogs, home, i18n,
    import_report, json, layers, live, menus, missing_fonts, playground, plugins_ui, present, tabs, updates_ui, versions,
};

#[cfg(test)]
thread_local! {
    static RUN_LOG: std::cell::RefCell<Vec<(String, Value)>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
impl FigApp {
    pub(crate) fn clear_run_log() {
        RUN_LOG.with_borrow_mut(Vec::clear);
    }

    pub(crate) fn runs_of(command: &str) -> Vec<Value> {
        RUN_LOG.with_borrow(|log| log.iter().filter(|(id, _)| id == command).map(|(_, params)| params.clone()).collect())
    }
}

/// An import started from the interface names its layers in the language of the interface.
fn with_role_names(id: &str, mut params: Value) -> Value {
    if id == "uigen.importComponent"
        && let Some(map) = params.as_object_mut().filter(|m| !m.contains_key("roleNames"))
    {
        let names = [
            ("Background", i18n::tr("Background")),
            ("Label", i18n::tr("Label")),
            ("Icon", i18n::tr("Icon")),
            ("Content", i18n::tr("Content")),
            ("Shape", i18n::tr("Shape")),
            ("Button", i18n::tr("Button")),
            ("Link", i18n::tr("Link")),
            ("Input", i18n::tr("Input")),
            ("Image", i18n::tr("Image")),
            ("List", i18n::tr("List")),
            ("Item", i18n::tr("Item")),
            ("Heading", i18n::tr("Heading")),
        ];
        let names: serde_json::Map<String, Value> = names.into_iter().map(|(role, name)| (role.to_string(), Value::String(name.to_string()))).collect();
        map.insert("roleNames".into(), Value::Object(names));
    }
    params
}

impl FigApp {
    /// Dismiss the composer after a successful switch away from the Comment tool.
    pub(crate) fn clear_comment_draft_after_tool_set(&mut self, id: &str, succeeded: bool) {
        if id == "tool.set" && succeeded && self.session.tool.tool != figcraft_engine::tools::ToolId::Comment {
            self.ui.comments.draft = None;
        }
    }

    /// In a library project `page.add` without a name or a template names the new component after the UI language.
    fn with_component_template(&self, id: &str, mut params: Value) -> Value {
        if id == "page.add"
            && self.session.in_lib_project()
            && let Some(map) = params.as_object_mut().filter(|m| !m.contains_key("name") && !m.contains_key("template"))
        {
            map.insert("template".into(), Value::String(i18n::tr("Component {n}").to_string()));
        }
        params
    }

    /// Run a command, surfacing errors as a toast. Returns the result.
    pub fn run(&mut self, id: &str, params: Value) -> Option<Value> {
        let params = with_role_names(id, self.with_component_template(id, params));
        #[cfg(test)]
        RUN_LOG.with_borrow_mut(|log| log.push((id.to_string(), params.clone())));
        let layers_generation = self.session.clipboard.layers_generation;
        let r = self.session.execute(id, &params);
        self.clear_comment_draft_after_tool_set(id, r.is_ok());
        let told_by_the_tab = playground::after_command(self, id, &r);
        self.handle_ui_requests();
        self.invalidate_synthetic_copy(layers_generation);
        // A command may change the file the window shows (`doc.switch`, `doc.nextTab`,
        // `file.open`/`file.close`). Record that presentation before returning, so an in-flight
        // screenshot admitted on the old file cannot accept pixels from the new one.
        self.observe_presentation();
        r.map_err(|e| {
            if !told_by_the_tab {
                self.failed(id, &e);
            }
        })
        .ok()
    }

    /// A programmatic command that changed the engine clipboard never wrote the system clipboard:
    /// forget the old system mirror so a later user paste does not trust it as our own copy.
    /// `before` is the layers generation captured before the command ran.
    pub(crate) fn invalidate_synthetic_copy(&mut self, before: u64) {
        if self.control_synthetic && self.session.clipboard.layers_generation != before {
            self.ui.last_copy = None;
        }
    }

    /// Record the file the window drawing now shows, with a monotonic generation that changes on
    /// every switch (an A→B→A switch is two generations, never the admitted one again).
    pub(crate) fn observe_presentation(&mut self) {
        let window = self.windows.current;
        self.windows.presentation(&self.session, window);
    }

    /// [`run`](Self::run) for something the user did (a dialog, a drop, a paste, a recent file):
    /// a file it imports shows its import report. Programmatic calls use `run` and never open one.
    pub fn run_user(&mut self, id: &str, params: Value) -> Option<Value> {
        let result = self.run(id, params.clone());
        if let Some(result) = &result {
            import_report::capture(self, id, &params, result);
        }
        missing_fonts::check_later(self, id, result.as_ref());
        result
    }

    /// Show command `id`'s error.
    pub(crate) fn failed(&mut self, id: &str, e: &figcraft_engine::EngineError) {
        let msg = e.to_string();
        log::warn!("{id}: {msg}");
        self.show_error(msg);
    }

    /// Show an error as a toast, kept as the last error.
    pub(crate) fn show_error(&mut self, msg: String) {
        self.toast(msg.clone());
        self.ui.last_error = Some(msg);
    }

    /// Run a command that may talk to a server without waiting for it (the engine's
    /// [`start_background`](Session::start_background)): `done` gets the result, at once or when
    /// the answer arrives (the network wakes the window then); errors show as with [`run`](Self::run).
    /// [`Session::waiting_for`] tells the panels to show that it's on its way.
    pub fn run_background(&mut self, ctx: &egui::Context, id: &str, params: Value, done: Done) {
        self.waiting.retain(|(c, _)| c != id);
        let r = self.session.start_background(id, params);
        self.handle_ui_requests();
        match r {
            Ok(Some(v)) => done(self, ctx, v),
            Ok(None) => self.waiting.push((id.to_string(), done)),
            Err(e) => self.failed(id, &e),
        }
    }

    /// Stop waiting for command `id` (Cancel, or its window closed): its answer is dropped.
    pub fn cancel_background(&mut self, id: &str) {
        self.session.cancel_background(id);
        self.waiting.retain(|(c, _)| c != id);
    }

    /// Hand the results of background commands that finished to what's waiting for them.
    fn background_done(&mut self, ctx: &egui::Context) {
        while let Some((id, r)) = self.session.take_background_done() {
            self.handle_ui_requests();
            let done = self.waiting.iter().position(|(c, _)| *c == id).map(|i| self.waiting.remove(i).1);
            match (r, done) {
                (Ok(v), Some(done)) => done(self, ctx, v),
                (Ok(_), None) => {}
                (Err(e), _) => self.failed(&id, &e),
            }
        }
    }

    /// Dev Mode (the Inspect tab): the canvas measures on hover without Alt and edits nothing;
    /// entering it ends editing text or vectors.
    pub(super) fn sync_dev_mode(&mut self) {
        let dev = self.ui.right_tab == RightTab::Inspect;
        if dev && !self.session.tool.dev_mode {
            let _ = self.session.drop_gesture();
            let editing = self.session.ds().map(|d| (d.text_edit.is_some(), d.vector_edit.is_some())).unwrap_or_default();
            for (on, cmd) in [(editing.0, "text.edit.end"), (editing.1, "vector.edit.end")] {
                if on {
                    self.run(cmd, json!({}));
                }
            }
        }
        self.session.tool.dev_mode = dev;
    }

    /// Act on UI requests queued by commands (dialogs, panels).
    pub fn handle_ui_requests(&mut self) {
        let reqs = std::mem::take(&mut self.session.ui_requests);
        for r in reqs {
            match r.kind.as_str() {
                "dialog.about" => {
                    self.ui.about = true;
                    self.ui.about_tab = dialogs::AboutTab::About;
                }
                "dialog.preferences" => self.ui.preferences = true,
                "dialog.missingFonts" => missing_fonts::request(self),
                "export.lit" => dialogs::lit_export::request(self),
                "dialog.projectSettings" => dialogs::project_settings::request(self),
                "dialog.importComponent" => dialogs::import_component::request(self, r.data.get("package").and_then(Value::as_str)),
                "panel.shortcuts" => self.ui.shortcuts_panel = !self.ui.shortcuts_panel,
                "palette.actions" => menus::toggle_palette(&mut self.ui),
                "panel.design" if self.session.doc().is_ok_and(figcraft_engine::board::is_board) => self.ui.board_panel = !self.ui.board_panel,
                "panel.design" => self.ui.right_tab = RightTab::Design,
                "board.editText" => {
                    if let Some(id) = r.data.get("id").and_then(figcraft_engine::params::id_of)
                        && let Err(e) = figcraft_engine::board::edit_text(&mut self.session, id)
                    {
                        self.toast(e.to_string());
                    }
                }
                "panel.prototype" => self.ui.right_tab = RightTab::Prototype,
                "panel.compare" if compare_tab::available(self) => self.ui.right_tab = RightTab::Compare,
                "panel.designPrototype" => {
                    self.ui.right_tab = if self.ui.right_tab == RightTab::Prototype { RightTab::Design } else { RightTab::Prototype };
                }
                "panel.devMode" => {
                    self.ui.right_tab = if self.ui.right_tab == RightTab::Inspect { RightTab::Design } else { RightTab::Inspect };
                }
                "panel.leftSidebar" => {
                    let shown = if self.ui.narrow { self.ui.mobile_panel == Some(MobilePanel::Left) } else { !self.ui.left_collapsed };
                    if shown {
                        self.ui.close_left_panel();
                    } else {
                        self.ui.open_left_panel();
                    }
                }
                "panel.widths" => {
                    let width = |k: &str| r.data.get(k).and_then(Value::as_f64).map(|w| w as f32);
                    let (left, right) = (width("left"), width("right"));
                    let reset = left.is_none() && right.is_none();
                    self.set_panel_widths(
                        left.unwrap_or(if reset { PANEL_DEFAULT } else { self.ui.left_width }),
                        right.unwrap_or(if reset { PANEL_DEFAULT } else { self.ui.right_width }),
                    );
                    self.services.save_prefs(&self.session.prefs);
                }
                "focus.region" => self.ui.nav.step(r.data.get("step").and_then(Value::as_i64).unwrap_or(1).clamp(-4, 4) as i32),
                "panel.layers" => self.ui.left_tab = LeftTab::File,
                "panel.assets" => {
                    self.ui.left_tab = LeftTab::Assets;
                    self.ui.open_left_panel();
                    self.ui.versions = Default::default();
                }
                "panel.versionHistory" => versions::open(self),
                "view.devCompare" => {
                    self.ui.dev.compare = dev_compare::CompareView::requested(&r.data);
                }
                "panel.libraries" => {
                    self.ui.left_tab = LeftTab::Assets;
                    self.ui.open_left_panel();
                    self.ui.libraries = true;
                }
                "home" => self.ui.home = true,
                "home.templates" => {
                    self.ui.home = true;
                    self.ui.home_view.section = home::Section::Templates;
                }
                "home.gettingStarted" => {
                    self.ui.home = true;
                    self.ui.home_view.section = home::Section::Recents;
                    self.services.save_prefs(&self.session.prefs);
                }
                "dialog.saveAs" => self.save_as_dialog(),
                "dialog.projectOpen" => self.open_project_dialog(false),
                "dialog.projectNew" => self.open_project_dialog(true),
                "dialog.colorProfile" => self.ui.color_profile = self.session.doc().ok().map(|d| d.color_profile),
                "present" => {
                    if let Some(start) = r.data.get("start").and_then(figcraft_engine::params::id_of) {
                        tabs::open_present(self, start);
                    }
                }
                "present.options" => {
                    let v = &mut self.ui.present_view;
                    if let Some(s) = r.data.get("scale").and_then(|s| s.as_str()).and_then(present::PresentScale::parse) {
                        v.scale = s;
                    }
                    if let Some(f) = r.data.get("deviceFrame").and_then(|f| f.as_bool()) {
                        v.device_frame = f;
                    }
                    if let Some(h) = r.data.get("hotspotHints").and_then(|h| h.as_bool()) {
                        v.hotspot_hints = h;
                    }
                    if let Some(i) = r.data.get("inspect").and_then(|i| i.as_bool()) {
                        v.inspect = i;
                    }
                    if let Some(p) = self.ui.present.as_mut() {
                        p.view.device_frame = v.device_frame;
                        p.view.hotspot_hints = v.hotspot_hints;
                        p.view.inspect = v.inspect;
                        p.set_scale(v.scale);
                    }
                }
                "preview" => {
                    if let Some(start) = r.data.get("start").and_then(figcraft_engine::params::id_of) {
                        self.ui.preview = Some(present::Pane::new(start, &self.session));
                    }
                }
                "preview.close" => self.ui.preview = None,
                "open.help" => self.services.open_url("https://github.com/Xafree/ui-gen"),
                "layers.collapseAll" => layers::collapse_all(self),
                "contextMenu" => self.ui.key_menu = true,
                "selectArea" => self.start_key_box(),
                "canvas.renameLabel" => {
                    self.ui.rename_label = r.data.get("id").and_then(figcraft_engine::params::id_of).map(|id| layers::Rename { id, start: true });
                }
                "layers.rename" => {
                    let ids: Vec<NodeId> =
                        r.data.get("ids").and_then(Value::as_array).into_iter().flatten().filter_map(figcraft_engine::params::id_of).collect();
                    if ids.len() > 1 {
                        dialogs::open_batch_rename(self, &ids);
                    } else {
                        layers::start_rename(self, &ids);
                    }
                }
                "comment.open" => {
                    if let Some(id) = r.data.get("id").and_then(Value::as_u64) {
                        self.ui.comments.show(id);
                    }
                }
                "comment.new" => {
                    let x = r.data.get("x").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let y = r.data.get("y").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    comments::start_draft(self, x, y);
                }
                "dialog.unsaved" => self.toast(i18n::tr("This file has unsaved changes")),
                "dialog.plugins" => self.ui.plugins_window = true,
                "dialog.pluginSave" => plugins_ui::save_file(self, &r.data),
                "dialog.cursorChat" => live::open_chat(self),
                "window.new" | "window.focus" | "window.fullscreen" => self.window_requests.push(r),
                _ => {}
            }
        }
        if let Some(m) = self.session.message.take() {
            self.toast(m);
        }
    }

    /// Work safe from a logic-only callback (`App::logic`, which also runs while hidden) and from
    /// a normal UI pass: consumed control work, live polling, background completions, autosave and
    /// recovery deadlines. It never reads UI input, so it must not scan events, run shortcuts or
    /// lay out anything.
    pub fn background_tick(&mut self, ctx: &egui::Context) {
        crate::control::background(self, ctx);
        dialogs::project_settings::pump(self);
        // What happened in a live session (the network woke us for it).
        if self.session.live_poll() {
            if let Some(m) = self.session.message.take() {
                self.toast(live::failure_text(&m));
            }
            self.background_done(ctx);
        }
        // Files that arrived without a command (joined, recovered) take the kept View toggles too.
        self.session.apply_view_prefs();
        let now = self.clock.now();
        self.autosave_tick(ctx, now);
        self.auto_save_tick(ctx, now);
        updates_ui::tick(self, ctx);
        self.logic_ticked = true;
    }
}
