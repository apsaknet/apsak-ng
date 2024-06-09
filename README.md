# `apsaK NG`

[<img alt="github" src="https://img.shields.io/badge/github-aspectron/apsak--ng-8da0cb?style=for-the-badge&labelColor=555555&color=8da0cb&logo=github" height="20">](https://github.com/aspectron/apsak-ng)
<img src="https://img.shields.io/badge/platform-native-informational?style=for-the-badge&color=50a0f0" height="20">
<img src="https://img.shields.io/badge/platform-wasm32-informational?style=for-the-badge&color=50a0f0" height="20">
<img src="https://img.shields.io/github/actions/workflow/status/aspectron/apsak-ng/ci.yaml?style=for-the-badge" height="20">

<p align="center" style="margin:32px auto 0px auto;text-align:center;font-size:10px;color:#888;">
<img src="https://aspectron.org/images/projects/apsak-ng-screen-01.png" style="display:block;max-height:320px;max-width:524px;width:524px;height:auto;object-fit:cover;margin: 0px auto 0px auto;"><br/><sup>RUSTY APSAK P2P NODE &bull; APSAK WALLET &bull; BLOCKDAG VISUALIZER</sup></p>

<p align="center" style="margin:4px 0px;text-align:center;font-size:10px;color:#800;">
&bull; BETA RELEASE &bull;
</p>

## Features

This software incorporates the following functionality:
- Rusty apsaK p2p Node
- apsaK wallet based on the Rusty apsaK SDK
- Rusty apsaK CLI wallet
- BlockDAG visualizer
- Remote node connectivity

This project is built on top of and incorporates the [Rusty apsaK](https://github.com/apsaknet/rusty-apsak) core framework.

This software is ideological in nature with a strong focus on architecture and decentralization. It is a unified codebase tightly coupled with the Rusty apsaK project. Fully written in Rust, it is available as a high-performance desktop application on all major operating systems (Windows, Linux and MacOS) as well as in major web browsers. It does not rely on any JavaScript or Web frameworks, which greatly strengthens its security profile. The Web Browser extension based on this infrastructure is currently under development.

You can find more information about this project at [https://aspectron.org/en/projects/apsak-ng.html](https://aspectron.org/en/projects/apsak-ng.html).

## Releases

- You can obtain the latest binary redistributables from the [Releases](https://github.com/apsaknet/apsak-ng/releases) page.
- You can access the official Web App online at [https://ng.apsak.org](https://ng.apsak.org).

## Building

To build this project, you need to be able to build Rusty apsaK. If you have not built Rusty apsaK before, please follow the Rusty apsaK [build instructions](https://github.com/apsaknet/rusty-apsak/blob/master/README.md).

In addition, on linux, you need to install the following dependencies:

#### Ubuntu/Debian:
```bash
sudo apt-get update
sudo apt-get install libglib2.0-dev libatk1.0-dev libgtk-3-dev librust-atk-dev
```

#### Fedora:
```bash
sudo dnf install glib2-devel atk-devel gtk3-devel
```

Once you have Rusty apsaK built, you will be able to build and run this project as follows:

### From GitHub repository:

#### Running as Native App
```bash
cargo run --release
```

#### Running as Web App
```bash
cargo install trunk
trunk serve --release
```
Access via [https://localhost:8080](https://localhost:8080)

While the application is a static serve, you can not load it from the local file system due to CORS restrictions. Due to this, a web server is required. This application is designed to be built with [Trunk](https://trunkrs.dev/) and is served from the `dist/` folder.  This is a self-contained client-side application - once the application is loaded, the web server is no longer required.

#### Running Headless

apsaK NG application binary can be started in 3 ways:
- `apsak-ng` - starts apsaK NG in the default desktop mode
- `apsak-ng --daemon [rusty-apsak arguments]` - starts apsaK NG as a Rusty apsaK p2p node daemon
- `apsak-ng --cli` - starts apsaK NG as a Rusty apsaK CLI wallet

If you need access to the wallet in a headless environment, you can start apsaK NG in daemon mode and then use the CLI wallet to access the wallet.

## License

Licensed under a [PROPRIETARY MIT-style Open Source LICENSE](LICENSE) with the following restrictions: 
_You are expressly prohibited from using, adapting, or integrating this software into any cryptocurrency network or related technology other than the specified intended network for which it is developed - The apsaK BlockDAG cryptocurrency network._

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, shall be licensed as above, without any
additional terms or conditions.

## Donations

If you are a apsaK investor, please consider supporting this project. The funds will be used to cover operational costs and further the project's functionality. 

`apsak:qrwsj38ulfq30dwze7q5rvwy8rfa237ct9eegtexah3wdjgd7g5ggmw7ut4tu`
