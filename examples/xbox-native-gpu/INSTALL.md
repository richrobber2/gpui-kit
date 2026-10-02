# Install on Xbox using a phone

This is a development app for an Xbox Series X|S in Developer Mode. A Windows PC in your home is not required. A Windows cloud runner must first build the package.

## Build in GitHub

1. Use the `xbox-prototype` branch of `richrobber2/gpui-kit`. The source lives in `examples/xbox-native-gpu`; it builds with the rest of GPUI Kit present.
2. In the repository, open **Actions → Build GPUI Kit Xbox prototype → Run workflow**, selecting `xbox-prototype`. Existing account Actions quotas and billing apply.
3. Wait for a successful run. Download its **gpui-kit-xbox-x64** artifact to your phone and extract the ZIP.
4. Find the main `XboxGpu...x64...appx` and any supplied `Dependencies/x64/*.appx` files. The `.cer` is the public development certificate. No private signing key is included.

## Install through Xbox Device Portal

1. Boot the Xbox into Dev Mode and sign in. Open **Dev Home → Remote Access Settings**.
2. Enable Xbox Device Portal and require authentication. Set a username/password locally; you do not need to send those credentials to ChatGPT.
3. On your phone, connected to the same LAN, open the exact HTTPS URL shown in Dev Home's Remote Access section. Verify it is the console's local address before accepting its self-signed certificate notice. Do not expose this portal to the public internet.
4. Sign in and use **Add / Deploy app**. Select the main APPX, then supply the x64 dependency APPXs if requested. Portal labels vary by OS version. Do not choose an APPXUPLOAD store-submission file.
5. In Dev Home, select the installed app, open **View details**, and set **App type → Game**. Launch it.

## Verified command deployment

On 2026-10-02 the APPX update path on the test console returned success but left
an application that could not launch. The loose-folder workflow launched the
same executable successfully. Use this command from the repository with the
original downloaded artifact ZIP (not the inner APPX):

```bash
python3 examples/xbox-native-gpu/scripts/deploy.py gpui-kit-xbox-x64.zip \
  --portal https://YOUR-XBOX:11443 --insecure
```

`--insecure` accepts the local console’s self-signed TLS certificate. Supply
`--username YOUR-USER` if the portal requires authentication; the command prompts
for the password without putting it in command arguments. `--dry-run` validates
the artifact without contacting Xbox.

The command checks that the portal identifies itself as Xbox, validates the
package identity, uploads a folder named from the APPX hash, registers it, and
launches the lab. It stops only an existing Xbox GPU Lab process. It preserves
other apps and their files. Keep the uploaded development folder: the registered
app runs from those files. An unsuccessful launch must still be diagnosed from
the screen and LocalState report; a successful HTTP response alone is insufficient.

## Run

The demo runs a verified 256×256 calculation automatically when it opens.

- **D-pad / keyboard arrows:** select 64, 128 or 256. Selection starts at 256.
- **A / keyboard Enter:** run the selected matrix multiplication.
- **X / keyboard X or Space:** 128×128.
- **Y:** 64×64.
- Each operation checks every output against a single-thread C++ reference.
- The display reports the actual app memory limit, not a browser hint. No large allocation stress test is performed.
- The displayed GPU time includes buffer creation, uploads, dispatch, synchronization and readback. It excludes device/shader initialization. One sample is not a reliable throughput benchmark.

The app writes `native-gpu-result.json` into its own LocalState folder. Use the app's files view in Device Portal if your console exposes it; otherwise send a photo of the result screen. Errors are also written to `error.txt` when local storage remains available.

The GPUI interface uses DirectX 12; compute remains Direct3D 11 hardware. GPUI rendering and the initial compute job have been verified on Xbox Series X; physical controller input and suspension/resume remain experimental. When reporting a launch failure, include `LocalState/error.txt` and Device Portal diagnostics if available.

## Troubleshooting

- Missing dependency: include the x64 dependency packages from the same artifact.
- Reported budget around 1 GB: verify App type is Game and relaunch. The app displays what the OS actually reports; it does not promise a particular budget.
- New build cannot update the old installation: each build currently uses a fresh development certificate with the same publisher name. Uninstall the previous development app and install the new package. This deletes that app's local reports.
- Device/signature/deployment error: preserve the exact error text. Do not disable authentication or alter the Xbox system clock.
- Compute/device error: close and relaunch the app. If the graphics driver remains unavailable, restart the console. This app deliberately refuses a software graphics fallback.

## Status of this kit

The portable CPU tests passed. The Windows cloud workflow compiles the C++ and HLSL, builds the package, and verifies its signature before uploading the installation artifact. The command above was exercised from the phone: GPUI rendered, both initial and repeat launches verified all 65,536 GPU outputs, and the OS reported a 5 GiB memory budget. The measured single-sample GPU total was about 18 ms, including allocation/transfers/synchronization; this is not evidence of a throughput speedup. The source-kit ZIP is not an installable package; use the APPX inside the successful build artifact.
