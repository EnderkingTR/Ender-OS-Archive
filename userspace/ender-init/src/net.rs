//! Linux ağ arayüzü ve yönlendirme tablosu yapılandırması (`lo`, `eth0`).
//! Harici `ip`/`ifconfig` araçlarına ihtiyaç duymadan doğrudan soket `ioctl`
//! sistem çağrılarını (`SIOCSIFADDR`, `SIOCSIFNETMASK`, `SIOCSIFFLAGS`, `SIOCADDRT`) kullanır.

use std::io;
use std::mem;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::ptr;

const SIOCADDRT: libc::c_ulong = 0x890B;
const SIOCGIFFLAGS: libc::c_ulong = 0x8913;
const SIOCSIFFLAGS: libc::c_ulong = 0x8914;
const SIOCSIFADDR: libc::c_ulong = 0x8916;
const SIOCSIFNETMASK: libc::c_ulong = 0x891C;

const RTF_UP: libc::c_ushort = 0x0001;
const RTF_GATEWAY: libc::c_ushort = 0x0002;

#[repr(C)]
struct IfReqAddr {
    ifr_name: [u8; 16],
    addr: libc::sockaddr_in,
    _pad: [u8; 8],
}

#[repr(C)]
struct IfReqFlags {
    ifr_name: [u8; 16],
    flags: libc::c_short,
    _pad: [u8; 22],
}

fn make_ifr_name(iface: &str) -> io::Result<[u8; 16]> {
    let bytes = iface.as_bytes();
    if bytes.is_empty() || bytes.len() >= 16 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "geçersiz ağ arayüzü adı",
        ));
    }
    let mut name = [0u8; 16];
    name[..bytes.len()].copy_from_slice(bytes);
    Ok(name)
}

fn make_sockaddr_in(ip: [u8; 4]) -> libc::sockaddr_in {
    // SAFETY: `sockaddr_in` yalnızca tam sayı ve bayt dizisi alanları içeren bir C yapısıdır.
    let mut sa: libc::sockaddr_in = unsafe { mem::zeroed() };
    sa.sin_family = libc::AF_INET as libc::sa_family_t;
    sa.sin_port = 0;
    sa.sin_addr = libc::in_addr {
        s_addr: u32::from_ne_bytes(ip),
    };
    sa
}

fn copy_sockaddr_in(dst: &mut libc::sockaddr, ip: [u8; 4]) {
    let sa = make_sockaddr_in(ip);
    // SAFETY: `sockaddr` ve `sockaddr_in` ikisi de 16 baytlık UAPI soket adresleridir.
    unsafe {
        ptr::copy_nonoverlapping(
            (&raw const sa).cast::<u8>(),
            (dst as *mut libc::sockaddr).cast::<u8>(),
            mem::size_of::<libc::sockaddr_in>(),
        );
    }
}

fn open_inet_dgram_socket() -> io::Result<OwnedFd> {
    // SAFETY: AF_INET/SOCK_DGRAM standart bir IPv4 kontrol soketi açar.
    let fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        // SAFETY: `fd >= 0` yeni açılmış ve sahipliği bize ait geçerli bir dosya tanıtıcısıdır.
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

pub fn configure_interface(iface: &str, ip: [u8; 4], mask: [u8; 4]) -> io::Result<()> {
    let sock = open_inet_dgram_socket()?;
    let fd = sock.as_raw_fd();
    let ifr_name = make_ifr_name(iface)?;

    let mut req_addr = IfReqAddr {
        ifr_name,
        addr: make_sockaddr_in(ip),
        _pad: [0u8; 8],
    };
    // SAFETY: `fd` geçerli bir soket tanıtıcısı ve `req_addr` 40 baytlık `ifreq` düzenindedir.
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCSIFADDR, &raw mut req_addr) } < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut req_mask = IfReqAddr {
        ifr_name,
        addr: make_sockaddr_in(mask),
        _pad: [0u8; 8],
    };
    // SAFETY: `req_mask` geçerli bir 40 baytlık `ifreq` yapısıdır.
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCSIFNETMASK, &raw mut req_mask) } < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut req_flags = IfReqFlags {
        ifr_name,
        flags: 0,
        _pad: [0u8; 22],
    };
    // SAFETY: Mevcut arayüz bayraklarını okur.
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCGIFFLAGS, &raw mut req_flags) } < 0 {
        return Err(io::Error::last_os_error());
    }

    req_flags.flags |= (libc::IFF_UP | libc::IFF_RUNNING) as libc::c_short;
    // SAFETY: Arayüzü `IFF_UP | IFF_RUNNING` ile ayağa kaldırır.
    if unsafe { libc::syscall(libc::SYS_ioctl, fd, SIOCSIFFLAGS, &raw mut req_flags) } < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(())
}

pub fn add_default_gateway(gw: [u8; 4]) -> io::Result<()> {
    let sock = open_inet_dgram_socket()?;
    // SAFETY: `rtentry` sıfırlanmış bir C yapısıdır; alanları geçerli IPv4 adresleriyle doldurulur.
    let mut rt: libc::rtentry = unsafe { mem::zeroed() };
    copy_sockaddr_in(&mut rt.rt_dst, [0, 0, 0, 0]);
    copy_sockaddr_in(&mut rt.rt_genmask, [0, 0, 0, 0]);
    copy_sockaddr_in(&mut rt.rt_gateway, gw);
    rt.rt_flags = RTF_UP | RTF_GATEWAY;

    // SAFETY: `sock` geçerli bir soket, `rt` geçerli bir `rtentry` işaretçisidir.
    let rc = unsafe { libc::syscall(libc::SYS_ioctl, sock.as_raw_fd(), SIOCADDRT, &raw mut rt) };
    if rc < 0 {
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            return Ok(());
        }
        return Err(err);
    }
    Ok(())
}

/// `lo` ve QEMU SLIRP `eth0` ağını yapılandırır.
pub fn setup_networking() -> bool {
    if let Err(err) = configure_interface("lo", [127, 0, 0, 1], [255, 0, 0, 0]) {
        eprintln!("UYARI: lo arayüzü yapılandırılamadı: {err}");
    }

    if let Err(err) = configure_interface("eth0", [10, 0, 2, 15], [255, 255, 255, 0]) {
        eprintln!("UYARI: eth0 arayüzü yapılandırılamadı: {err}");
        return false;
    }

    if let Err(err) = add_default_gateway([10, 0, 2, 2]) {
        eprintln!("UYARI: varsayılan ağ geçidi (10.0.2.2) eklenemedi: {err}");
    }

    true
}
