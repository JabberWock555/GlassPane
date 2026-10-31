# Running Home Assistant + glasspane on an old Android phone

An old phone makes a surprisingly good home server:

- it has a built-in battery, so it rides out short power cuts like a UPS
- it has Wi-Fi
- it uses about 2 W
- it costs nothing if it's already in a drawer

This guide sets one up headless, with the screen off, running:

- **Home Assistant** (the official container image, run with udocker)
- **glasspane** (native build)
- **SSH** for remote administration
- **Tailscale** for secure access from outside your home

It was put together on a Xiaomi Mi A2 (Android 10, 4 GB RAM). Any arm64
phone on Android 7 or later with 3 GB+ of RAM should work.

> **Root is not needed.** Everything runs inside Termux as a normal app.
> The trade-offs: Home Assistant runs under `proot` emulation, so it is
> slower than on a Raspberry Pi. Integrations that need USB radios, Bluetooth
> or mDNS discovery may not work. Wi-Fi and cloud integrations (most smart
> plugs, fans and lights, e.g. Tuya and Wiz) work fine.

## 1. Prepare the phone

1. **Factory reset (optional).** Sign in with a throwaway Google account, or
   none, and remove apps you don't need.
2. **Connect it to Wi-Fi** and give it a **DHCP reservation** in your router,
   so its IP never changes.
3. **Turn on Developer options.** In Developer options, enable **Stay awake**
   (while charging) for setup, and **USB debugging** if you want to control it
   from a PC with [scrcpy](https://github.com/Genymobile/scrcpy).
4. **Install Termux, Termux:Boot and Termux:API from F-Droid or the
   [Termux GitHub releases](https://github.com/termux/termux-app/releases)**,
   not from Google Play: the Play builds are outdated. All three must come
   from the same source.
5. **Allow background running.** Go to Settings → Apps → Termux → Battery
   and choose **Unrestricted** (not optimized). Do the same for Termux:Boot,
   and later for Tailscale. Open Termux:Boot once so Android registers it.

## 2. Base packages, SSH and services

In Termux:

```bash
pkg update && pkg upgrade -y
pkg install -y openssh termux-services termux-api udocker git curl rust
termux-setup-storage          # lets Termux read /sdcard (screensaver photos)
```

`termux-services` brings **runit**: each service is a folder under
`$PREFIX/var/service/<name>` with a `run` script. runit restarts a service if
it crashes, and services keep running after you close the Termux window.
Restart Termux once after installing it.

**SSH** listens on port **8022** and only accepts key logins:

```bash
mkdir -p ~/.ssh && chmod 700 ~/.ssh
echo "ssh-ed25519 AAAA... you@pc" >> ~/.ssh/authorized_keys    # your PC's public key
chmod 600 ~/.ssh/authorized_keys
sv-enable sshd
whoami                                                    # your Termux user name
```

From the PC: `ssh -p 8022 <termux-user>@<phone-ip>`. You can now do the rest
from a comfortable keyboard.

**Start at boot:** Termux:Boot runs the scripts in `~/.termux/boot/`.

```bash
mkdir -p ~/.termux/boot
cat > ~/.termux/boot/start-services <<'EOF'
#!/data/data/com.termux/files/usr/bin/sh
termux-wake-lock                                   # keep the CPU awake with the screen off
. $PREFIX/etc/profile.d/start-services.sh          # start runit and every enabled service
EOF
chmod +x ~/.termux/boot/start-services
```

`termux-wake-lock` is what lets the phone keep serving with the screen off.
Termux shows a persistent notification while it holds the lock. That's
expected.

## 3. Home Assistant in udocker

[udocker](https://github.com/indigo-dc/udocker) runs Docker images without
root or a Docker daemon, by using `proot`. The official Home Assistant image
works in its **P1** mode.

```bash
udocker pull homeassistant/home-assistant:stable
udocker create --name=hass homeassistant/home-assistant:stable
udocker setup --execmode=P1 hass
mkdir -p ~/hass-config
```

Run it as a runit service. The image's normal init system doesn't work under
proot, so the service starts Home Assistant's Python module directly:

```bash
mkdir -p $PREFIX/var/service/hass/log
ln -sf $PREFIX/share/termux-services/svlogger $PREFIX/var/service/hass/log/run
cat > $PREFIX/var/service/hass/run <<'EOF'
#!/data/data/com.termux/files/usr/bin/sh
exec 2>&1
exec udocker run --entrypoint="" --volume=$HOME/hass-config:/config --env=TZ=Europe/Amsterdam \
  hass python3 -m homeassistant --config /config
EOF
chmod +x $PREFIX/var/service/hass/run
sv-enable hass
tail -f $PREFIX/var/log/sv/hass/current        # first start takes several minutes
```

Set `TZ` to your own time zone. When the log settles, open
`http://<phone-ip>:8123` from another device and create your account.

**Notes:**

- **If Home Assistant won't start this way,** the image layout may have
  changed. The [Termux-Udocker](https://github.com/George-Seven/Termux-Udocker)
  project keeps up-to-date scripts for running the official image under
  udocker.

- **Use `sv` to manage it** (`sv status hass`, `sv restart hass`). Don't also
  start Home Assistant by hand: two copies fighting over the same config
  folder cause odd errors.
- **The container shares the phone's network.** HA may still warn that it's
  "running in a container without host networking". That warning comes from
  the Docker image, not your setup.
- **HACS:** run the HACS download script inside the container, then restart
  HA and add the HACS integration:

  ```bash
  udocker run --entrypoint="" --volume=$HOME/hass-config:/config hass \
    sh -c 'cd /config && wget -O - https://get.hacs.xyz | bash -'
  sv restart hass
  ```

  Then go to Settings → Devices & services → Add integration → HACS. If HACS
  doesn't appear, clear the browser cache.

## 4. glasspane

On the phone:

```bash
git clone https://github.com/JabberWock555/glasspane ~/glasspane
cd ~/glasspane
cp glasspane.example.toml glasspane.toml && nano glasspane.toml   # your rooms and entities
bash scripts/install-termux.sh
```

The script:

1. installs Rust, curl and termux-services if needed
2. builds a release binary (the first build takes about 10 minutes on a phone)
3. creates `~/.config/glasspane/env` (mode 600) for your token
4. registers a `glasspane` runit service
5. creates `/sdcard/Pictures/Glasspane` for screensaver photos

Then add your token and restart:

```bash
nano ~/.config/glasspane/env        # HA_TOKEN=<long-lived access token>
sv restart glasspane
tail -f $PREFIX/var/log/sv/glasspane/current
```

Open `http://<phone-ip>:8090` on the tablet.

**To update later:**

```bash
bash ~/glasspane/scripts/update-termux.sh
```

This pulls with git (or unpacks a zip you pass it), keeps your
`glasspane.toml`, rebuilds incrementally and restarts the service.

**Screensaver photos:** copy pictures into `Pictures/Glasspane` on the phone,
e.g. over USB, or with `scp -P 8022 *.jpg <user>@<phone-ip>:/sdcard/Pictures/Glasspane/`.

## 5. Remote access with Tailscale

glasspane and Home Assistant should never be exposed directly to the
internet. [Tailscale](https://tailscale.com) gives your own devices a
private, encrypted network instead, with no port forwarding:

1. **Install Tailscale** from the Play Store on the phone, and sign in.
2. **Install it on each device** you want to reach home from, such as your
   own phone and laptop, signed in to the same account.
3. **Rename the server.** In the [admin console](https://login.tailscale.com/admin/machines),
   rename the phone to something like `ha-server`. With MagicDNS on, it's
   reachable at `http://ha-server:8123` and `http://ha-server:8090` from
   anywhere.
4. **Disable key expiry** for the server in the admin console, so it never
   silently drops off the network.
5. **Make the VPN permanent.** On the phone, go to Settings → Network →
   VPN → Tailscale and turn on **Always-on VPN**. Set Tailscale's battery
   setting to **Unrestricted**.

Only devices that need to reach home *from outside* need Tailscale. The wall
tablet is on your Wi-Fi, so it uses the LAN address and needs nothing extra.
That's handy, because an Android 4 tablet can't run Tailscale anyway.

## 6. Running it with the screen off

- **Screen off:** once SSH works, unplug USB. Keep the phone on a charger,
  and press power to turn the screen off. The wake lock keeps Termux
  running.
- **Battery health:** if your phone or a smart plug supports it, limit
  charging to about 80 %. Phones that sit at 100 % for months can swell.
- **After a reboot,** Termux:Boot starts `sshd`, `hass` and `glasspane` by
  itself. Check with:

  ```bash
  sv status sshd hass glasspane
  ```

## Troubleshooting

| Symptom | Check |
|---|---|
| Services stop when the screen turns off | Battery settings for Termux and Termux:Boot are **Unrestricted**. The wake-lock notification is showing. |
| Nothing starts after a reboot | Termux:Boot was opened once. `~/.termux/boot/start-services` is executable. |
| Home Assistant very slow to start | Normal under proot on the first run (several minutes). Watch `$PREFIX/var/log/sv/hass/current`. |
| Log says "token rejected" or "HA_TOKEN is not set" | The token in `~/.config/glasspane/env`. Then run `sv restart glasspane`. |
| Tablet shows "Still connecting to the server…" | The phone's IP or port 8090 from the tablet's browser. Both devices on the same Wi-Fi. |
| No AQI shown | `curl` is installed (`pkg install curl`). HA has a home location set, or set `aqi_location`. |
