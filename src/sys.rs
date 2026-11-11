//! Integracion con el sistema: bandeja, barra de tareas y arrastrar/soltar.
use crate::core::{Core, Event};
use std::sync::Arc;

pub fn urls_in(text: &str) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    for w in text.split_whitespace() {
        let w = w.trim_matches(|c| c == '"' || c == '\'' || c == '<' || c == '>');
        if (w.starts_with("http://") || w.starts_with("https://")) && !v.iter().any(|x| x == w) {
            v.push(w.to_string());
        }
    }
    v
}

const VIDEO_HOSTS: [&str; 18] = [
    "youtube.com", "youtu.be", "vimeo.com", "tiktok.com", "twitter.com", "x.com", "instagram.com", "facebook.com",
    "fb.watch", "twitch.tv", "soundcloud.com", "dailymotion.com", "bilibili.com", "reddit.com", "bandcamp.com",
    "kick.com", "rumble.com", "music.youtube.com",
];

/// Heuristica para el portapapeles: solo enlaces de sitios de video/audio conocidos.
pub fn looks_like_media_url(u: &str) -> bool {
    let rest = u.trim_start_matches("https://").trim_start_matches("http://");
    let host = rest.split(['/', '?']).next().unwrap_or("").to_lowercase();
    VIDEO_HOSTS.iter().any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

// ---------------------------------------------------------------- bandeja

pub enum TrayAction {
    Show,
    Quit,
}

pub struct Tray {
    _icon: tray_icon::TrayIcon,
    quit_id: tray_icon::menu::MenuId,
    show_id: tray_icon::menu::MenuId,
}

impl Tray {
    pub fn new(show_label: &str, quit_label: &str) -> Option<Tray> {
        use tray_icon::menu::{Menu, MenuItem};
        let menu = Menu::new();
        let show = MenuItem::new(show_label, true, None);
        let quit = MenuItem::new(quit_label, true, None);
        menu.append_items(&[&show, &quit]).ok()?;
        let img = image::load_from_memory(include_bytes!("../assets/logo.png")).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let icon = tray_icon::Icon::from_rgba(img.into_raw(), w, h).ok()?;
        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false)
            .with_tooltip("ClipVault")
            .with_icon(icon)
            .build()
            .map_err(|e| log::warn!("bandeja no disponible: {e}"))
            .ok()?;
        Some(Tray { _icon: tray, quit_id: quit.id().clone(), show_id: show.id().clone() })
    }

    pub fn poll(&self) -> Vec<TrayAction> {
        let mut out = Vec::new();
        while let Ok(ev) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            if ev.id == self.quit_id {
                out.push(TrayAction::Quit);
            } else if ev.id == self.show_id {
                out.push(TrayAction::Show);
            }
        }
        while let Ok(ev) = tray_icon::TrayIconEvent::receiver().try_recv() {
            match ev {
                tray_icon::TrayIconEvent::Click { button: tray_icon::MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. }
                | tray_icon::TrayIconEvent::DoubleClick { .. } => out.push(TrayAction::Show),
                _ => {}
            }
        }
        out
    }
}

// ---------------------------------------------------------------- barra de tareas + arrastrar (Windows)

#[cfg(windows)]
pub mod win {
    use super::*;
    use windows::core::implement;
    use windows::Win32::Foundation::{HWND, POINTL};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, IDataObject, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, DVASPECT_CONTENT, FORMATETC,
        TYMED_HGLOBAL,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::{
        IDropTarget, IDropTarget_Impl, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY,
    };
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::Shell::{ITaskbarList3, TaskbarList, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL};

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut core::ffi::c_void)
    }

    pub struct Taskbar {
        t: Option<ITaskbarList3>,
    }

    impl Taskbar {
        pub fn new() -> Taskbar {
            // SAFETY: COM se inicializa en el hilo de la UI, que es donde se usa despues.
            let t = unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                CoCreateInstance::<_, ITaskbarList3>(&TaskbarList, None, CLSCTX_INPROC_SERVER).ok()
            };
            if let Some(t) = &t {
                unsafe {
                    let _ = t.HrInit();
                }
            }
            Taskbar { t }
        }

        /// None = sin progreso; Some(f) con f<0 = indeterminado; si no, 0..1
        pub fn set(&self, window: isize, progress: Option<f32>) {
            let Some(t) = &self.t else { return };
            let h = hwnd(window);
            unsafe {
                match progress {
                    None => {
                        let _ = t.SetProgressState(h, TBPF_NOPROGRESS);
                    }
                    Some(f) if f < 0.0 => {
                        let _ = t.SetProgressState(h, TBPF_INDETERMINATE);
                    }
                    Some(f) => {
                        let _ = t.SetProgressState(h, TBPF_NORMAL);
                        let _ = t.SetProgressValue(h, (f.clamp(0.0, 1.0) * 1000.0) as u64, 1000);
                    }
                }
            }
        }
    }

    #[implement(IDropTarget)]
    struct DropTarget {
        core: Arc<Core>,
    }

    impl DropTarget {
        fn text_of(data: &IDataObject) -> Option<String> {
            let fmt = FORMATETC {
                cfFormat: CF_UNICODETEXT.0,
                ptd: std::ptr::null_mut(),
                dwAspect: DVASPECT_CONTENT.0,
                lindex: -1,
                tymed: TYMED_HGLOBAL.0 as u32,
            };
            // SAFETY: lectura de un HGLOBAL de texto UTF-16 terminado en NUL, liberado con ReleaseStgMedium.
            unsafe {
                let mut medium = data.GetData(&fmt).ok()?;
                let ptr = GlobalLock(medium.u.hGlobal) as *const u16;
                let text = if ptr.is_null() {
                    None
                } else {
                    let mut len = 0;
                    while *ptr.add(len) != 0 && len < 65_536 {
                        len += 1;
                    }
                    Some(String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len)))
                };
                let _ = GlobalUnlock(medium.u.hGlobal);
                ReleaseStgMedium(&mut medium);
                text
            }
        }
    }

    #[allow(non_snake_case)]
    impl IDropTarget_Impl for DropTarget_Impl {
        fn DragEnter(&self, _data: windows::core::Ref<'_, IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            unsafe { *effect = DROPEFFECT_COPY };
            Ok(())
        }

        fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            unsafe { *effect = DROPEFFECT_COPY };
            Ok(())
        }

        fn DragLeave(&self) -> windows::core::Result<()> {
            Ok(())
        }

        fn Drop(&self, data: windows::core::Ref<'_, IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
            unsafe { *effect = DROPEFFECT_COPY };
            if let Some(d) = data.as_ref() {
                if let Some(text) = DropTarget::text_of(d) {
                    let urls = urls_in(&text);
                    if urls.is_empty() {
                        self.core.toast("Lo soltado no contiene un enlace");
                    } else {
                        self.core.push_event(Event::Dropped(urls));
                    }
                }
            }
            Ok(())
        }
    }

    /// Sustituye el destino de arrastre de winit por uno que acepta texto/enlaces.
    pub fn register_drop(window: isize, core: Arc<Core>) {
        let target: IDropTarget = DropTarget { core }.into();
        let h = hwnd(window);
        unsafe {
            let _ = RevokeDragDrop(h);
            if let Err(e) = RegisterDragDrop(h, &target) {
                log::warn!("arrastrar y soltar no disponible: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        assert_eq!(urls_in("mira https://youtu.be/x y \"https://a.com/b\""), vec!["https://youtu.be/x", "https://a.com/b"]);
        assert!(looks_like_media_url("https://www.youtube.com/watch?v=1"));
        assert!(!looks_like_media_url("https://github.com/x"));
    }
}
