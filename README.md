<a id="readme-top"></a>

<!-- PROJECT SHIELDS -->
[![Contributors][contributors-shield]][contributors-url]
[![Forks][forks-shield]][forks-url]
[![Stargazers][stars-shield]][stars-url]
[![Issues][issues-shield]][issues-url]
[![License][license-shield]][license-url]

<!-- PROJECT LOGO -->
<br />
<div align="center">
  <h3 align="center">Staked Contract</h3>

  <p align="center">
    Anchor-based Solana staking and creator-fee routing program for Pump.fun-launched tokens.
    <br />
    <a href="programs/staking/README.md"><strong>Explore the docs &raquo;</strong></a>
    <br />
    <br />
    <a href="programs/staking/src">View Source</a>
    &middot;
    <a href="https://github.com/iceypump/staked-contract/issues/new?labels=bug">Report Bug</a>
    &middot;
    <a href="https://github.com/iceypump/staked-contract/issues/new?labels=enhancement">Request Feature</a>
  </p>
</div>

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#installation">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#roadmap">Roadmap</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>

<!-- ABOUT THE PROJECT -->
## About The Project

Staked Contract is an Anchor workspace for `staked`, a Solana program that lets token holders stake
Pump.fun-launched tokens for fixed lock periods and receive rewards from creator-fee streams.

The program uses a per-mint `fee_owner` PDA so Pump.fun and PumpSwap creator fees can be routed into
staking rewards through bounded on-chain instructions instead of being controlled by a human wallet.
It supports legacy SPL Token and Token-2022 staking mints, SOL rewards, quote-token rewards, WSOL
unwrapping, and protocol-fee routing.

Current status:

- Program name: `staked`
- Program ID: `3skopQVdqns5x5GjU2c3S4nEcmVbDTkMoZWRaVLsJrAa`
- Anchor version: `0.32.1`
- Network target: Solana
- Audit status: unaudited

See [programs/staking/README.md](programs/staking/README.md) for account layouts, PDA derivations,
staking rules, reward flows, and instruction details.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Built With

* [![Rust][Rust]][Rust-url]
* [![Solana][Solana]][Solana-url]
* [![Anchor][Anchor]][Anchor-url]
* [![Node.js][Node.js]][Node-url]

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

Follow these steps to build and check the program locally.

### Prerequisites

Install the Solana and Anchor development toolchain:

* Rust
  ```sh
  rustc --version
  ```
* Solana CLI
  ```sh
  solana --version
  ```
* Anchor CLI
  ```sh
  anchor --version
  ```
* Node.js and npm
  ```sh
  node --version
  npm --version
  ```

### Installation

1. Clone the repo
   ```sh
   git clone https://github.com/iceypump/staked-contract.git
   ```
2. Enter the project directory
   ```sh
   cd staked-contract
   ```
3. Install script dependencies
   ```sh
   npm install
   ```
4. Build the Anchor program
   ```sh
   anchor build
   ```
5. Run tests
   ```sh
   anchor test
   ```

For a lighter Rust-only check:

```sh
cargo check
```

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- USAGE EXAMPLES -->
## Usage

Useful development commands:

```sh
npm run check:scripts
npm run create:pump-devnet -- --quote sol
npm run volume:pump-devnet -- --mint <MINT>
npm run stake:test-wallets -- --mint <MINT>
```

The helper scripts default to Solana Devnet and `~/.config/solana/id.json`. Generated local test
wallets are written to `.devnet-test-wallets/`, which is intentionally ignored.

The Anchor program source is in `programs/staking/src`. The program-specific README documents the
main instructions:

- `initialize_launch`
- `stake`
- `increase_stake`
- `claim_rewards`
- `initialize_quote_rewards`
- `initialize_quote_stake`
- `unstake`
- `claim_pump_creator_fees`
- `claim_pump_quote_creator_fees`
- `claim_pumpswap_creator_fees`
- `claim_pumpswap_quote_creator_fees`
- `claim_protocol_fees`
- `claim_quote_protocol_fees`

_For more details, see the [program documentation](programs/staking/README.md)._

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ROADMAP -->
## Roadmap

- [ ] Add complete integration tests
- [ ] Add localnet Pump.fun and PumpSwap mocks
- [ ] Add quote-token reward tests
- [ ] Add Token-2022 staking tests
- [ ] Add deployment guide
- [ ] Add independent audit report
- [ ] Add client SDK examples

See the [open issues](https://github.com/iceypump/staked-contract/issues) for a full list of
proposed features and known issues.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTRIBUTING -->
## Contributing

Contributions are welcome. If you have a suggestion that would make this better, fork the repo and
open a pull request, or open an issue with the `enhancement` label.

1. Fork the project
2. Create your feature branch
   ```sh
   git checkout -b feature/amazing-feature
   ```
3. Commit your changes
   ```sh
   git commit -m "Add amazing feature"
   ```
4. Push to the branch
   ```sh
   git push origin feature/amazing-feature
   ```
5. Open a pull request

<p align="right">(<a href="#readme-top">back to top</a>)</p>

### Top Contributors

<a href="https://github.com/iceypump/staked-contract/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=iceypump/staked-contract" alt="contrib.rocks image" />
</a>

<!-- LICENSE -->
## License

Distributed under the MIT License.

See [LICENSE](LICENSE) for more information.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTACT -->
## Contact

(X) Iceypump - [@icetypump](https://x.com/iceyypump)

Project Link: [https://github.com/iceypump/staked-contract](https://github.com/iceypump/staked-contract)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ACKNOWLEDGMENTS -->
## Acknowledgments

* [Anchor](https://www.anchor-lang.com/)
* [Solana](https://solana.com/docs)
* [Pump.fun public docs](https://github.com/pump-fun/pump-public-docs)
* [Best README Template](https://github.com/othneildrew/Best-README-Template)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- MARKDOWN LINKS & IMAGES -->
[contributors-shield]: https://img.shields.io/github/contributors/iceypump/staked-contract.svg?style=for-the-badge
[contributors-url]: https://github.com/iceypump/staked-contract/graphs/contributors
[forks-shield]: https://img.shields.io/github/forks/iceypump/staked-contract.svg?style=for-the-badge
[forks-url]: https://github.com/iceypump/staked-contract/network/members
[stars-shield]: https://img.shields.io/github/stars/iceypump/staked-contract.svg?style=for-the-badge
[stars-url]: https://github.com/iceypump/staked-contract/stargazers
[issues-shield]: https://img.shields.io/github/issues/iceypump/staked-contract.svg?style=for-the-badge
[issues-url]: https://github.com/iceypump/staked-contract/issues
[license-shield]: https://img.shields.io/github/license/iceypump/staked-contract.svg?style=for-the-badge
[license-url]: LICENSE
[Rust]: https://img.shields.io/badge/Rust-000000?style=for-the-badge&logo=rust&logoColor=white
[Rust-url]: https://www.rust-lang.org/
[Solana]: https://img.shields.io/badge/Solana-14F195?style=for-the-badge&logo=solana&logoColor=black
[Solana-url]: https://solana.com/
[Anchor]: https://img.shields.io/badge/Anchor-663399?style=for-the-badge
[Anchor-url]: https://www.anchor-lang.com/
[Node.js]: https://img.shields.io/badge/Node.js-339933?style=for-the-badge&logo=nodedotjs&logoColor=white
[Node-url]: https://nodejs.org/
