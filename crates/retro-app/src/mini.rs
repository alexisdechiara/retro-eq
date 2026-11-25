//! La mini-fenêtre posée au-dessus du jeu pendant un test (ADR-0008).
//!
//! Elle ne doit jamais prendre le clavier au jeu. Sous Windows, elle est donc créée une
//! fois au lancement, marquée « ne s'active pas », « toujours au-dessus » et « absente de
//! la barre des tâches », puis montrée et cachée par Windows directement, sans l'activer.
//! Ailleurs (Linux, pour les essais), c'est une fenêtre ordinaire.

use crate::MiniFenetre;
use slint::ComponentHandle;

pub struct Mini {
    fenetre: MiniFenetre,
    visible: bool,
    #[cfg(windows)]
    prete: bool,
}

impl Mini {
    pub fn nouvelle() -> Result<Mini, slint::PlatformError> {
        Ok(Mini {
            fenetre: MiniFenetre::new()?,
            visible: false,
            #[cfg(windows)]
            prete: false,
        })
    }

    /// La fenêtre Slint, pour que le pont y recopie l'état du test.
    pub fn fenetre(&self) -> &MiniFenetre {
        &self.fenetre
    }

    /// À appeler au lancement, avant d'afficher la fenêtre principale : sous Windows, la
    /// mini-fenêtre est créée et stylée à ce moment-là, quand aucun jeu n'a encore le focus.
    pub fn initialiser(&mut self) {
        #[cfg(windows)]
        {
            self.prete = self.preparer();
        }
    }

    /// Montre ou cache la mini-fenêtre. Sans effet si elle est déjà dans cet état.
    pub fn montrer(&mut self, visible: bool) {
        if visible == self.visible {
            return;
        }
        self.visible = visible;
        self.appliquer(visible);
    }

    #[cfg(not(windows))]
    fn appliquer(&mut self, visible: bool) {
        let _ = if visible {
            self.fenetre.show()
        } else {
            self.fenetre.hide()
        };
    }

    #[cfg(windows)]
    fn appliquer(&mut self, visible: bool) {
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_SHOWNOACTIVATE};
        if !self.prete {
            self.prete = self.preparer();
        }
        if let Some(hwnd) = self.poignee() {
            // SAFETY: `hwnd` est la poignée d'une fenêtre vivante de ce fil (elle vient de
            // winit à l'instant) ; ShowWindow ne garde aucun pointeur.
            let _ = unsafe { ShowWindow(hwnd, if visible { SW_SHOWNOACTIVATE } else { SW_HIDE }) };
        }
    }

    /// La poignée Win32 de la fenêtre, une fois que Slint l'a créée.
    #[cfg(windows)]
    fn poignee(&self) -> Option<windows::Win32::Foundation::HWND> {
        use i_slint_backend_winit::WinitWindowAccessor;
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        self.fenetre
            .window()
            .with_winit_window(|w| match w.window_handle().ok()?.as_raw() {
                RawWindowHandle::Win32(h) => Some(windows::Win32::Foundation::HWND(
                    h.hwnd.get() as *mut core::ffi::c_void
                )),
                _ => None,
            })
            .flatten()
    }

    /// Crée la vraie fenêtre (Slint ne le fait qu'au premier affichage), lui donne ses
    /// styles et la place en bas à droite de la zone de travail de l'écran principal.
    // A VERIFIER SUR LE PC : pas de vol de focus au premier affichage, placement avec une
    // mise à l'échelle de 125 % ou 150 %, tenue au-dessus d'un jeu en plein écran fenêtré.
    #[cfg(windows)]
    fn preparer(&mut self) -> bool {
        use windows::Win32::Foundation::RECT;
        use windows::Win32::UI::WindowsAndMessaging::{
            GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, ShowWindow, SystemParametersInfoW,
            GWL_EXSTYLE, HWND_TOPMOST, SPI_GETWORKAREA, SWP_NOACTIVATE, SWP_NOSIZE, SW_HIDE,
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        };
        if self.fenetre.show().is_err() {
            return false;
        }
        let Some(hwnd) = self.poignee() else {
            return false;
        };
        let taille = self.fenetre.window().size();
        let marge = 24;
        let mut zone = RECT::default();
        // SAFETY: `hwnd` est une fenêtre vivante de ce fil. `zone` est un RECT valide qui
        // vit pendant tout l'appel à SystemParametersInfoW, seul à y écrire. Les autres
        // appels ne prennent que des valeurs.
        unsafe {
            let _ = ShowWindow(hwnd, SW_HIDE);
            let styles = GetWindowLongPtrW(hwnd, GWL_EXSTYLE)
                | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST).0 as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, styles);
            let trouve = SystemParametersInfoW(
                SPI_GETWORKAREA,
                0,
                Some(&mut zone as *mut RECT as *mut core::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
            .is_ok();
            let (x, y) = if trouve {
                (
                    zone.right - taille.width as i32 - marge,
                    zone.bottom - taille.height as i32 - marge,
                )
            } else {
                (marge, marge)
            };
            let _ = SetWindowPos(
                hwnd,
                Some(HWND_TOPMOST),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
        }
        true
    }
}
