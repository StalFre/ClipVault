//! Notificaciones nativas de Windows (toast).

pub fn toast(title: &str, body: &str) {
    #[cfg(windows)]
    {
        let (t, b) = (title.to_string(), body.to_string());
        std::thread::spawn(move || {
            let r = tauri_winrt_notification::Toast::new(tauri_winrt_notification::Toast::POWERSHELL_APP_ID)
                .title(&t)
                .text1(&b)
                .show();
            if let Err(e) = r {
                log::warn!("notificación fallida: {e}");
            }
        });
    }
    #[cfg(not(windows))]
    let _ = (title, body);
}
