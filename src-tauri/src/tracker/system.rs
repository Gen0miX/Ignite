//! System probes: running processes and user idle time.

use std::time::Instant;

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

use super::matcher::ProcessInfo;

/// Lists running processes with their executable path (when readable).
pub struct ProcessProbe {
    system: Option<System>,
    own_pid: u32,
}

impl ProcessProbe {
    pub fn new() -> Self {
        let system = match System::new() {
            Ok(s) => Some(s),
            Err(e) => {
                tracing::warn!(error = %e, "process listing unavailable: automatic tracking disabled");
                None
            }
        };
        Self {
            system,
            own_pid: std::process::id(),
        }
    }

    pub fn processes(&mut self) -> Vec<ProcessInfo> {
        let Some(system) = self.system.as_mut() else {
            return Vec::new();
        };
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        system
            .processes()
            .values()
            .filter(|p| p.pid().as_u32() != self.own_pid)
            .map(|p| ProcessInfo {
                pid: p.pid().as_u32(),
                name: p.name().to_string_lossy().into_owned(),
                exe_path: p.exe().map(|e| e.display().to_string()),
            })
            .collect()
    }
}

impl Default for ProcessProbe {
    fn default() -> Self {
        Self::new()
    }
}

/// Seconds since the last keyboard, mouse or gamepad input.
///
/// Keyboard and mouse come from `GetLastInputInfo`; gamepads are not seen by
/// that API, so XInput packet numbers are compared between calls.
pub struct IdleProbe {
    last_packets: [Option<u32>; 4],
    last_pad_input: Option<Instant>,
}

impl IdleProbe {
    pub fn new() -> Self {
        Self {
            last_packets: [None; 4],
            last_pad_input: None,
        }
    }

    pub fn idle_seconds(&mut self) -> i64 {
        let keyboard_mouse = keyboard_mouse_idle_secs();
        if self.poll_gamepads() {
            self.last_pad_input = Some(Instant::now());
        }
        let pad = self.last_pad_input.map(|t| t.elapsed().as_secs() as i64);
        match (keyboard_mouse, pad) {
            (Some(k), Some(p)) => k.min(p),
            (Some(k), None) => k,
            (None, Some(p)) => p,
            // Unknown: assume active rather than discounting play time.
            (None, None) => 0,
        }
    }

    /// True when any connected controller reported a new input state.
    #[cfg(windows)]
    fn poll_gamepads(&mut self) -> bool {
        use windows_sys::Win32::UI::Input::XboxController::{XInputGetState, XINPUT_STATE};
        let mut changed = false;
        for (index, last) in self.last_packets.iter_mut().enumerate() {
            let mut state: XINPUT_STATE = unsafe { std::mem::zeroed() };
            // SAFETY: valid pointer to a zeroed XINPUT_STATE, index in 0..4.
            let result = unsafe { XInputGetState(index as u32, &mut state) };
            if result != 0 {
                *last = None; // not connected
                continue;
            }
            if last.is_some_and(|p| p != state.dwPacketNumber) {
                changed = true;
            }
            *last = Some(state.dwPacketNumber);
        }
        changed
    }

    #[cfg(not(windows))]
    fn poll_gamepads(&mut self) -> bool {
        false
    }
}

impl Default for IdleProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
fn keyboard_mouse_idle_secs() -> Option<i64> {
    use windows_sys::Win32::System::SystemInformation::GetTickCount;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` is a properly sized LASTINPUTINFO.
    if unsafe { GetLastInputInfo(&mut info) } == 0 {
        return None;
    }
    // Both are 32-bit millisecond tick counts: wrapping_sub handles the 49-day rollover.
    let now = unsafe { GetTickCount() };
    Some(i64::from(now.wrapping_sub(info.dwTime)) / 1000)
}

#[cfg(not(windows))]
fn keyboard_mouse_idle_secs() -> Option<i64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_processes_without_ourselves() {
        let mut probe = ProcessProbe::new();
        let list = probe.processes();
        assert!(!list.is_empty());
        assert!(list.iter().all(|p| p.pid != std::process::id()));
    }

    #[test]
    fn idle_time_is_never_negative() {
        let mut probe = IdleProbe::new();
        assert!(probe.idle_seconds() >= 0);
    }
}
