# Bluetooth controlled cord light switch

**Before building, read this carefully (especially Safety guidelines)**

This project aims to create a remote-controlled, *smart* cord light switch. It has both physical control button and Bluetooth LE capabilities, allowing for very natural user experience.

This repo contains both PCB design and firmware.

Hardware seem to work, however still barely tested in the field. High voltage part is rated at 220-230V / 50 Hz (European-ish standard). It should be trivial to recalculate the supply for US standard.

The firmware was never fully developed, but `firmware` directory contains the current work-in-progress state to get you started.

**Make sure you know what you're doing. This project involves working with high voltages and requires advanced electronics experience. I'm not taking any responsibility for possible damages linked to usage of this device**

![board render](/images/board.png)

## Bringup

There are 4 solder pads underneath the board. You'll have to Solder some wires to it in order to connect to the programming probe. Supply your own safe external power from your debugging probe or any other low-power source. **NEVER INTERFACE WITH THE DEVICE WHILE PLUGGED INTO MAINS POWER!**

The firmware is written in Rust and uses Embassy the async framework with nrf-softdevice crate for BLE.

## Some safety guidelines

- Never touch any parts of the board while operational - you'd likely get shock;
- Don't leave device unattended for now - it's not stable enough to trust it;
- Be generally in a good state of mind and follow what you are doing - **it is easy to make mistakes and burn your stuff**

## Roadmap

- Proper case design (I just used one from the real switch, with drilled-out insides);
- HomeAssistant integration.
