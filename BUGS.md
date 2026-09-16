# Initial Run (Sep 16, 2026)

## Run 1

### Test Environment
- Rootfs: local `./alpinefs/`
- Command: `kuro create alp-test ./alpinefs`

### Logs
```sh
[kuro-host] Spawned container init process with PID: 31012
[kuro-child] Error during initialization: Mount error: Failed to unmount /old_root: EINVAL: Invalid argument
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
