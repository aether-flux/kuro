### &Option<_>
- &Option<> is bad. Its lifetime gets over when the line getting that value is done executing.
- Fix: Use .as_ref() to convert &Option<_> into Option<&_>.

### uid/gid helpers
- setuidmap/setgidmap: Helper binaries with necessary capabilities enabled (SETUID/SETGID)
- setuid/setgid: When ran on unprivileged process, or going from root to dropping root privileges, sets all three (real, effective, saved set) uid/gid
- setresuid/setresgid: Explicitly sets real, effective, saved set uid/gid

### kill()
- Sending kill signal to pid=0 sends signal to every process in the caller's process group
- Sending kill signal to pid=-1 sends signal to every single process on the system that the user has permission to kill (so as sudo, every process ever)
