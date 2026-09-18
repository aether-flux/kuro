## Run 1

### Test Environment
- Rootfs: local `./alpinefs/`
- Command: `kuro create alp-test ./alpinefs`

### Logs
```sh
[kuro-host] Spawned container init process with PID: 6553
[kuro] Error during initialization: Mount failed for target '/dev': EINVAL: Invalid argument
```

## Run 2

### Test Environment
- Rootfs: `./alpinefs/`
- Command: `kuro state alp-test`

### Logs
```sh
{
  "ociVersion": "1.3.0",
  "id": "alp-test",
  "status": "created",
  "pid": -1,
  "bundle": "/home/aether/Projects/kuro/alpinefs",
  "annotations": {}
}
```

## Run 4

### Test Env
- Rootfs: ./alpinefs/
- Command: kuro create alp-test ./alpinefs

### Logs
```sh
[kuro-host] Spawned container init process with PID: 9968
[kuro] Error during initialization: Mount failed for target '/sys/fs/cgroup': EPERM: Operation not permitted
[kuro] Command execution failed: Child initialization failed: Mount failed for target '/sys/fs/cgroup': EPERM: Operation not permitted
[kuro] Cleaning up container 'alp-test'...
[kuro] WARN: Failed to remove cgroup path "/sys/fs/cgroup/kuro/alp-test": Device or resource busy (os error 16)
Error: ExecFailed("Child initialization failed: Mount failed for target '/sys/fs/cgroup': EPERM: Operation not permitted")
```

## Run 5

### Test Env
- Rootfs: ./alpinefs/
- Command: kuro create alp-test ./alpinefs

### Logs
```sh
[kuro-host] Spawned container init process with PID: 11995
[kuro] Error during initialization: Mount failed for target '/dev/mqueue': EBUSY: Device or resource busy
[kuro] Command execution failed: Child initialization failed: Mount failed for target '/dev/mqueue': EBUSY: Device or resource busy
[kuro] Cleaning up container 'alp-test'...
[kuro] WARN: Failed to remove cgroup path "/sys/fs/cgroup/kuro/alp-test": Device or resource busy (os error 16)
Error: ExecFailed("Child initialization failed: Mount failed for target '/dev/mqueue': EBUSY: Device or resource busy")
```

## Run 6

### Test env
- Rootfs: ./alpinefs
- Command: kuro start alp-test

### Logs
```sh
state loaded
spec loaded
signal sent
status saved
Started container alp-test...
```

#### Insights
- Capabilities not working: logging permitted Capabilities, operation not supported
- Capabilities not working: logging permitted capabilities, operation not permitted:
  - panicked: Ok(Err(Capability("Error setting permitted capabilities: caps error: capset failure: Operation not permitted (os error 1)")))


# Fixed Bugs
- Cleanup kills every single process
- EPERM mount, invalid "/old_root" (replaced "." with &root)
- Container state being "created" even though it wasn't
- Child hangs on setup error because parent keeps listening for signal that'll never arrive
- Mount error, not able to mount /dev device with EINVAL
- Mount error, /sys/fs/cgroup/ EPERM Operation Not Permitted
- Mount error, /dev/mqueue/ EBUSY Device or resource busy
- Mount error, /dev/null no such file or directory (during masked path /proc/kcore)
- Start command does not do anything
  - Looks like container process dies after create command finishes.
  - But that's pure coincidence of timing. Actual issue is related to the named pipe (fifo).
  - It opens the file as read-only, and at that moment the writer count on it is 0. When later called a read(), rule is if there are currently 0 writers open, read() returns 0 (EOF) immediately. Clearing NONBLOCK doesn't matter.
- Fixing terminal with libc::dup2() results in create command hanging after opening pipe.
- Start command spawns shell:
  - Issue: I had to run the binary as sudo for necessary permissions for creating container
  - But sudo sets up its own pts (stdio handles)
  - When create runs, it set master as that sudo stdio and slave as container's stdio
  - After execution, since sudo ends, its stdio handles are also destroyed, thus leaving no master for the slave
  - Fix: isolate child's stdio handles in run_child_init after signalling parent that child setup is complete
- Capabilities not working: logging bounding Capabilities, operation not supported:
  - Bounding caps can't be set, but rather what we have to do is see which capabilities are not in target bounding caps and drop those individually.
