<div align="center">
  
  # Tiron
  
  **Reasonable Automation Engine**
</div>

<div align="center">
  <a href="https://github.com/lapce/tiron/actions/workflows/ci.yml" target="_blank">
    <img src="https://github.com/lapce/tiron/actions/workflows/ci.yml/badge.svg" />
  </a>
  <a href="https://discord.gg/GK4uSQMT4X" target="_blank">
    <img src="https://img.shields.io/discord/946858761413328946?logo=discord" />
  </a>
  <a href="https://github.com/lapce/tiron/releases" target="_blank">
    <img src="https://img.shields.io/github/v/release/lapce/tiron" />
  </a>
</div>

**Tiron** is an automation tool that's easy to use and aims to be as fast as possible. It’s agentless by using SSH and has a TUI for the outputs of the tasks. There is an example Tiron configuration [here](https://github.com/lapce/tiron/tree/main/examples/example_tiron_project).

<div align="center">
  <img width="894" alt="Screenshot" src="https://github.com/lapce/tiron/assets/1169480/0c53b83e-901b-410e-afc3-3a4aa4917b93">
</div>

## Features
* **No YAML:** Tiron uses a new configuration language called [rcl](https://github.com/ruuda/rcl), which is simple to write with some basic code functionalities.
* **Agentless:** By using SSH, Tiron connects to the remote machines without the need to install an agent first.
* **TUI:** Tiron has a built in terminal user interfaces to display the outputs of the running tasks.
* **Correctness:** Tiron pre validates all the rcl files and will throw errors before the task is started to execute.
* **Speed:** On validating all the input, Tiron also pre populates all the data for tasks, and send them to the remote machines in one go to save the roundtrips between the client and remote.  

## Quickstart

Run below to install latest Tiron binary to ```/usr/local/bin```
```bash
curl -sL https://tiron.run/install.sh | sh
```

## License
Tiron is licensed under the Apache 2.0 license.
