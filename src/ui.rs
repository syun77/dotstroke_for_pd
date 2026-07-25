//! egui-facing UI support. Native menu ownership lives here so the app does not
//! need to know how platform menu events are wired.
use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{Menu, MenuEvent, MenuId, MenuItem, Submenu};

pub struct NativeMenu {
    menu: Menu,
    new_id: MenuId,
    load_id: MenuId,
    save_id: MenuId,
}
impl NativeMenu {
    pub fn new() -> Self {
        let menu = Menu::new();
        let new_item = MenuItem::new(
            "New",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyN)),
        );
        let load_item = MenuItem::new(
            "Load JSON",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyO)),
        );
        let save_item = MenuItem::new(
            "Save JSON",
            true,
            Some(Accelerator::new(Some(Modifiers::SUPER), Code::KeyS)),
        );
        let file_menu = Submenu::with_items("File", true, &[&new_item, &load_item, &save_item])
            .expect("failed to create File menu");
        menu.append(&file_menu).expect("failed to append File menu");
        Self {
            menu,
            new_id: new_item.id().clone(),
            load_id: load_item.id().clone(),
            save_id: save_item.id().clone(),
        }
    }
    pub fn init(&self) {
        #[cfg(target_os = "macos")]
        self.menu.init_for_nsapp();
    }
    pub fn actions(&self) -> (bool, bool, bool) {
        let mut actions = (false, false, false);
        for event in MenuEvent::receiver().try_iter() {
            if event.id == self.new_id {
                actions.0 = true;
            } else if event.id == self.load_id {
                actions.1 = true;
            } else if event.id == self.save_id {
                actions.2 = true;
            }
        }
        actions
    }
}
