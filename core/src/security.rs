//! Human readable descriptions for the sandboxing options, used by the GUI to
//! render checkbox lists instead of free-form text boxes.

/// An option with a short human readable description.
#[derive(Debug, Clone, Copy)]
pub struct DescribedOption {
    pub id: &'static str,
    pub description: &'static str,
}

/// Linux capabilities systemd-nspawn understands.
pub const CAPABILITY_INFO: &[DescribedOption] = &[
    DescribedOption { id: "CAP_CHOWN", description: "Change ownership of files" },
    DescribedOption { id: "CAP_DAC_OVERRIDE", description: "Bypass file read/write/execute permission checks" },
    DescribedOption { id: "CAP_DAC_READ_SEARCH", description: "Bypass file read and directory search permissions" },
    DescribedOption { id: "CAP_FOWNER", description: "Bypass permission checks on file owner operations" },
    DescribedOption { id: "CAP_FSETID", description: "Keep setuid/setgid bits when modifying files" },
    DescribedOption { id: "CAP_KILL", description: "Send signals to any process" },
    DescribedOption { id: "CAP_SETGID", description: "Change group IDs" },
    DescribedOption { id: "CAP_SETUID", description: "Change user IDs" },
    DescribedOption { id: "CAP_SETPCAP", description: "Add or remove capabilities from other processes" },
    DescribedOption { id: "CAP_LINUX_IMMUTABLE", description: "Set the immutable and append-only file attributes" },
    DescribedOption { id: "CAP_NET_BIND_SERVICE", description: "Bind to low TCP/UDP ports (below 1024)" },
    DescribedOption { id: "CAP_NET_BROADCAST", description: "Send network broadcasts and listen to multicast" },
    DescribedOption { id: "CAP_NET_ADMIN", description: "Configure interfaces, routing and firewalls" },
    DescribedOption { id: "CAP_NET_RAW", description: "Use RAW and PACKET sockets" },
    DescribedOption { id: "CAP_IPC_LOCK", description: "Lock memory (mlock, huge pages)" },
    DescribedOption { id: "CAP_IPC_OWNER", description: "Bypass IPC ownership checks" },
    DescribedOption { id: "CAP_SYS_MODULE", description: "Load and unload kernel modules" },
    DescribedOption { id: "CAP_SYS_RAWIO", description: "Raw I/O ports and /dev/mem access" },
    DescribedOption { id: "CAP_SYS_CHROOT", description: "Call chroot()" },
    DescribedOption { id: "CAP_SYS_PTRACE", description: "Trace and inspect other processes" },
    DescribedOption { id: "CAP_SYS_PACCT", description: "Configure process accounting" },
    DescribedOption { id: "CAP_SYS_ADMIN", description: "Broad administration: mounts, namespaces, many ioctls" },
    DescribedOption { id: "CAP_SYS_BOOT", description: "Reboot or halt the system" },
    DescribedOption { id: "CAP_SYS_NICE", description: "Raise priority and set affinity of other processes" },
    DescribedOption { id: "CAP_SYS_RESOURCE", description: "Override resource limits" },
    DescribedOption { id: "CAP_SYS_TIME", description: "Set the system clock" },
    DescribedOption { id: "CAP_SYS_TTY_CONFIG", description: "Configure TTY devices" },
    DescribedOption { id: "CAP_MKNOD", description: "Create device nodes with mknod()" },
    DescribedOption { id: "CAP_LEASE", description: "Take file leases" },
    DescribedOption { id: "CAP_AUDIT_WRITE", description: "Write to the kernel audit log" },
    DescribedOption { id: "CAP_AUDIT_CONTROL", description: "Configure kernel auditing" },
    DescribedOption { id: "CAP_AUDIT_READ", description: "Read the audit log through netlink multicast" },
    DescribedOption { id: "CAP_SETFCAP", description: "Set file capabilities on binaries" },
    DescribedOption { id: "CAP_MAC_OVERRIDE", description: "Override Mandatory Access Control" },
    DescribedOption { id: "CAP_MAC_ADMIN", description: "Configure Mandatory Access Control" },
    DescribedOption { id: "CAP_SYSLOG", description: "Read the kernel log and /proc/kallsyms" },
    DescribedOption { id: "CAP_WAKE_ALARM", description: "Trigger wake-up alarms" },
    DescribedOption { id: "CAP_BLOCK_SUSPEND", description: "Block system suspend" },
    DescribedOption { id: "CAP_PERFMON", description: "Use performance monitoring (perf_event_open)" },
    DescribedOption { id: "CAP_BPF", description: "Use BPF (bpf())" },
    DescribedOption { id: "CAP_CHECKPOINT_RESTORE", description: "Checkpoint and restore processes" },
];

/// systemd "system call groups". Checking one emits ~@group, which removes the
/// whole group from the calls the container may make.
pub const SYSCALL_GROUP_INFO: &[DescribedOption] = &[
    DescribedOption { id: "@aio", description: "Asynchronous I/O (io_setup, io_submit)" },
    DescribedOption { id: "@basic-io", description: "Basic file descriptor I/O (read, write, lseek)" },
    DescribedOption { id: "@chown", description: "Change file ownership (chown, fchownat)" },
    DescribedOption { id: "@clock", description: "Adjust the system clock (settimeofday, adjtimex)" },
    DescribedOption { id: "@cpu-emulation", description: "Emulate CPU instructions not available natively" },
    DescribedOption { id: "@debug", description: "Debugging (ptrace, perf_event_open)" },
    DescribedOption { id: "@file-system", description: "File system operations (mount, open, stat)" },
    DescribedOption { id: "@io-event", description: "Event loop calls (epoll, poll, select)" },
    DescribedOption { id: "@ipc", description: "System V and POSIX IPC (shm, msg, sem)" },
    DescribedOption { id: "@keyring", description: "Kernel keyring management" },
    DescribedOption { id: "@memlock", description: "Lock memory (mlock, mlockall)" },
    DescribedOption { id: "@module", description: "Load and unload kernel modules" },
    DescribedOption { id: "@mount", description: "Mount and unmount file systems (mount, umount, pivot_root)" },
    DescribedOption { id: "@network-io", description: "Network I/O (socket, connect, sendto)" },
    DescribedOption { id: "@obsolete", description: "Obsolete and removed system calls" },
    DescribedOption { id: "@privileged", description: "Privileged operations (setuid, capset, chroot)" },
    DescribedOption { id: "@process", description: "Process management (fork, execve, wait)" },
    DescribedOption { id: "@raw-io", description: "Raw I/O port access (iopl, ioperm)" },
    DescribedOption { id: "@reboot", description: "Reboot, halt and power off" },
    DescribedOption { id: "@resources", description: "Resource limits and CPU scheduling (setrlimit, sched_setaffinity)" },
    DescribedOption { id: "@sandbox", description: "Sandboxing helpers (seccomp, landlock)" },
    DescribedOption { id: "@setuid", description: "Change user and group identity" },
    DescribedOption { id: "@signal", description: "Signal handling (kill, sigaction)" },
    DescribedOption { id: "@swap", description: "Enable and disable swap" },
    DescribedOption { id: "@sync", description: "Sync file systems (sync, fsync, syncfs)" },
    DescribedOption { id: "@system-service", description: "The set systemd considers appropriate for services" },
    DescribedOption { id: "@timer", description: "Timers and time (timer_create, nanosleep)" },
];

/// Look up the description of a capability.
pub fn capability_description(id: &str) -> Option<&'static str> {
    CAPABILITY_INFO.iter().find(|o| o.id == id).map(|o| o.description)
}

/// Look up the description of a system call group.
pub fn syscall_group_description(id: &str) -> Option<&'static str> {
    SYSCALL_GROUP_INFO.iter().find(|o| o.id == id).map(|o| o.description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_list_matches_the_validator() {
        let mut a: Vec<&str> = CAPABILITY_INFO.iter().map(|c| c.id).collect();
        let mut b: Vec<&str> = crate::validate::CAPABILITIES.to_vec();
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn descriptions_are_present() {
        assert!(CAPABILITY_INFO.iter().all(|c| !c.description.is_empty()));
        assert!(SYSCALL_GROUP_INFO.iter().all(|c| !c.description.is_empty()));
        assert!(capability_description("CAP_SYS_ADMIN").is_some());
        assert!(syscall_group_description("@mount").is_some());
    }
}
