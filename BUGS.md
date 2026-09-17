# Initial Run (Sep 16, 2026)

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
[kuro-host] Spawned container init process with PID: 13518
[kuro] Error during initialization: Mount failed for target '/dev': EINVAL: Invalid argument
[kuro] Command execution failed: Child initialization failed: Mount failed for target '/dev': EINVAL: Invalid argument
[kuro] Cleaning up container 'alp-test'...
[kuro] WARN: Failed to remove cgroup path "/sys/fs/cgroup/kuro/alp-test": Device or resource busy (os error 16)
Error: ExecFailed("Child initialization failed: Mount failed for target '/dev': EINVAL: Invalid argument")
```

#### Insights
- Mount error, not able to mount /dev device with EINVAL


# Fixed Bugs
- Cleanup kills every single process
- EPERM mount, invalid "/old_root" (replaced "." with &root)
- Container state being "created" even though it wasn't
- Child hangs on setup error because parent keeps listening for signal that'll never arrive
