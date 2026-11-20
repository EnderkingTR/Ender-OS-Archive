// SPDX-License-Identifier: GPL-2.0

//! Ender-OS v0.3 — Rust-for-Linux `/dev/ender` karakter aygıtı sürücüsü.
//!
//! Yüklenebilir çekirdek modülü (`.ko`) olarak derlenir. `/dev/ender` aygıt
//! düğümünü kaydeder; okuma, yazma ve özel `ioctl` istatistik/sıfırlama
//! işlemlerini çekirdek içi `Mutex` korumalı paylaşılan tampon üzerinde yürütür.

use core::pin::Pin;

use kernel::{
    c_str,
    device::Device,
    fs::{File, Kiocb},
    ioctl::{_IO, _IOC_SIZE, _IOR},
    iov::{IovIterDest, IovIterSource},
    miscdevice::{MiscDevice, MiscDeviceOptions, MiscDeviceRegistration},
    prelude::*,
    sync::aref::ARef,
    uaccess::UserSlice,
};

const BUF_CAPACITY: usize = 512;
const DEFAULT_MSG: &[u8] = b"Ender-OS Rust cekirdek modulu (/dev/ender) hazir!\n";

const ENDER_IOCTL_RESET: u32 = _IO('E' as u32, 0x01);
const ENDER_IOCTL_GET_OPENS: u32 = _IOR::<u64>('E' as u32, 0x02);
const ENDER_IOCTL_GET_WRITES: u32 = _IOR::<u64>('E' as u32, 0x03);
const ENDER_IOCTL_GET_READS: u32 = _IOR::<u64>('E' as u32, 0x04);

struct DeviceState {
    buf: [u8; BUF_CAPACITY],
    len: usize,
    open_count: u64,
    read_count: u64,
    write_count: u64,
}

impl DeviceState {
    const fn new() -> Self {
        Self {
            buf: [0u8; BUF_CAPACITY],
            len: 0,
            open_count: 0,
            read_count: 0,
            write_count: 0,
        }
    }

    fn reset(&mut self) {
        let n = DEFAULT_MSG.len();
        self.buf[..n].copy_from_slice(DEFAULT_MSG);
        self.len = n;
        self.read_count = 0;
        self.write_count = 0;
    }
}

kernel::sync::global_lock! {
    // SAFETY: `EnderDevModule::init` icinde `/dev/ender` kaydedilmeden once tam bir kez baslatilir.
    unsafe(uninit) static DEVICE_STATE: Mutex<DeviceState> = DeviceState::new();
}

module! {
    type: EnderDevModule,
    name: "ender_dev",
    authors: ["Ender-OS Contributors"],
    description: "Ender-OS Rust-for-Linux /dev/ender karakter aygiti surucusu",
    license: "GPL",
}

#[pin_data(PinnedDrop)]
struct EnderDevModule {
    #[pin]
    _miscdev: MiscDeviceRegistration<EnderDevice>,
}

impl kernel::InPlaceModule for EnderDevModule {
    fn init(_module: &'static ThisModule) -> impl PinInit<Self, Error> {
        // SAFETY: Modul baslatilirken tam bir kez ve aygit kaydedilmeden once cagrilir.
        unsafe { DEVICE_STATE.init() };
        DEVICE_STATE.lock().reset();

        pr_info!("ender_dev: Rust-for-Linux modulu yuklendi (/dev/ender)\n");

        let options = MiscDeviceOptions {
            name: c_str!("ender"),
        };

        try_pin_init!(Self {
            _miscdev <- MiscDeviceRegistration::register(options),
        })
    }
}

#[pinned_drop]
impl PinnedDrop for EnderDevModule {
    fn drop(self: Pin<&mut Self>) {
        pr_info!("ender_dev: modul kaldiriliyor (/dev/ender kapatildi)\n");
    }
}

#[pin_data]
struct EnderDevice {
    dev: ARef<Device>,
}

#[vtable]
impl MiscDevice for EnderDevice {
    type Ptr = Pin<KBox<Self>>;

    fn open(_file: &File, misc: &MiscDeviceRegistration<Self>) -> Result<Pin<KBox<Self>>> {
        let dev = ARef::from(misc.device());
        let opens = {
            let mut state = DEVICE_STATE.lock();
            state.open_count += 1;
            state.open_count
        };

        dev_info!(dev, "ender_dev: /dev/ender acildi (acilis={})\n", opens);

        KBox::try_pin_init(try_pin_init! { EnderDevice { dev } }, GFP_KERNEL)
    }

    fn read_iter(mut kiocb: Kiocb<'_, Self::Ptr>, iov: &mut IovIterDest<'_>) -> Result<usize> {
        let mut state = DEVICE_STATE.lock();
        let read = iov.simple_read_from_buffer(kiocb.ki_pos_mut(), &state.buf[..state.len])?;
        if read > 0 {
            state.read_count += 1;
        }
        Ok(read)
    }

    fn write_iter(mut kiocb: Kiocb<'_, Self::Ptr>, iov: &mut IovIterSource<'_>) -> Result<usize> {
        let me = kiocb.file();
        let mut state = DEVICE_STATE.lock();
        let written = iov.copy_from_iter(&mut state.buf);
        state.len = written;
        state.write_count += 1;
        *kiocb.ki_pos_mut() = 0;

        dev_info!(me.dev, "ender_dev: {} bayt yazildi\n", written);
        Ok(written)
    }

    fn ioctl(me: Pin<&EnderDevice>, _file: &File, cmd: u32, arg: usize) -> Result<isize> {
        match cmd {
            ENDER_IOCTL_RESET => {
                DEVICE_STATE.lock().reset();
                dev_info!(me.dev, "ender_dev: tampon ve sayaclar sifirlandi\n");
                Ok(0)
            }
            ENDER_IOCTL_GET_OPENS | ENDER_IOCTL_GET_WRITES | ENDER_IOCTL_GET_READS => {
                let val = {
                    let state = DEVICE_STATE.lock();
                    match cmd {
                        ENDER_IOCTL_GET_OPENS => state.open_count,
                        ENDER_IOCTL_GET_WRITES => state.write_count,
                        _ => state.read_count,
                    }
                };

                let mut writer = UserSlice::new(UserPtr::from_addr(arg), _IOC_SIZE(cmd)).writer();
                writer.write::<u64>(&val)?;
                Ok(0)
            }
            _ => Err(ENOTTY),
        }
    }
}
