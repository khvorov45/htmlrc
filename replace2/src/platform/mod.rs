#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux as os;

#[cfg(target_os = "windows")]
pub(crate) mod windows;
#[cfg(target_os = "windows")]
pub(crate) use windows as os;

#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;
#[cfg(target_arch = "x86_64")]
pub(crate) use x86_64 as arch;
