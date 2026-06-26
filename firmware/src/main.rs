//! A part of Nordic switch - Bluetooth controlled cord light switch project
//! This firmware and hardware is a work in progress - use at your own risk.

#![no_std]
#![no_main]

use assign_resources::assign_resources;
use defmt::{info, unwrap};
use embassy_nrf::peripherals;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::Timer;
use futures::future;
use git_version::git_version;
use heapless::String;

use embassy_nrf::{
    gpio::{self, Input},
    interrupt, Peripherals,
};
use nrf_softdevice::{
    ble::{self, advertisement_builder as advb, peripheral as blep},
    raw as nrf_defines, Softdevice,
};

use defmt_rtt as _;
use panic_probe as _;

// There are only 2 'resources' - still, good for consistency across projects :)
assign_resources! {
    button: ButtonResources {
        pin: P0_19,
    },
    triac: TriacResources {
        pin: P0_20,
    }
}

/// Actions used for triac control channel
#[derive(defmt::Format)]
enum TriacAction {
    On,
    Off,
    Toggle,
}

/// Channel used to send messages to triac
type TriacSignal = Signal<CriticalSectionRawMutex, TriacAction>;

/// Gatt configuration. A single 'custom' control service containing everything needed.
#[nrf_softdevice::gatt_service(uuid = "c831c2f2-817f-11ee-b962-0242ac120002")]
pub struct ControlService {
    /// Control point that controls the lamp
    #[characteristic(uuid = "c831c2f2-817f-11ee-b962-0242ac130002", read, write)]
    triac_control: bool,

    /// Version string of this firmware for debugging
    #[characteristic(uuid = "c831c2f2-817f-11ee-b962-0242ac140002", read)]
    version: String<32>,
}

/// A gatt server that holds all of our characteristics
#[nrf_softdevice::gatt_server]
pub struct GattServer {
    control: ControlService,
}

/// Runs advertisement cycle. Returns connection that we can feed to the gatt_server.
async fn advertise(softdevice: &Softdevice) -> Result<ble::Connection, blep::AdvertiseError> {
    static ADV_DATA: advb::LegacyAdvertisementPayload = advb::LegacyAdvertisementBuilder::new()
        .flags(&[advb::Flag::GeneralDiscovery, advb::Flag::LE_Only])
        .full_name("Nordic Switch")
        .build();

    static SCAN_DATA: advb::LegacyAdvertisementPayload =
        advb::LegacyAdvertisementBuilder::new().build();

    let packet = blep::ConnectableAdvertisement::ScannableUndirected {
        adv_data: &ADV_DATA,
        scan_data: &SCAN_DATA,
    };

    let config = blep::Config {
        interval: 1600, // 1s
        ..Default::default()
    };

    blep::advertise_connectable(softdevice, packet, &config).await
}

/// Task that handles all the bluetooth stuff
#[embassy_executor::task]
async fn bluetooth_task_run(softdevice: &'static mut Softdevice, triac: &'static TriacSignal) {
    let gatt = unwrap!(GattServer::new(softdevice));

    // Store the git version in the control service to help debug issues in the field
    unwrap!(gatt
        .control
        .version_set(&unwrap!(git_version!().try_into())));

    // That's how we handle all events coming from the gatt_server
    let event_handler = |e: GattServerEvent| match e {
        GattServerEvent::Control(event) => match event {
            ControlServiceEvent::TriacControlWrite(on) => triac.signal(if on {
                TriacAction::On
            } else {
                TriacAction::Off
            }),
        },
    };

    future::join(
        // Run connection cycle
        async {
            loop {
                if let Ok(connection) = advertise(softdevice).await {
                    ble::gatt_server::run(&connection, &gatt, event_handler).await;
                }
            }
        },
        // Run softdevice logic
        softdevice.run(),
    )
    .await;
}

/// Task that listens for triac signals and reacts appropriately
#[embassy_executor::task]
async fn triac_controller_run(res: TriacResources, signal: &'static TriacSignal) {
    // note logic level inversion here - we start with high level
    let mut triac = gpio::Output::new(res.pin, gpio::Level::High, gpio::OutputDrive::Standard);

    loop {
        let action = signal.wait().await;
        info!("received triac action '{}'", action);
        match action {
            TriacAction::On => triac.set_low(),
            TriacAction::Off => triac.set_high(),
            TriacAction::Toggle => triac.toggle(),
        }
    }
}

/// Task that handles button events and sends actions to triac task
#[embassy_executor::task]
async fn button_listener_run(res: ButtonResources, triac: &'static TriacSignal) {
    let mut button = Input::new(res.pin, gpio::Pull::Up);

    loop {
        button.wait_for_falling_edge().await;

        // do a second probe for debouncing
        Timer::after_millis(10).await;
        if button.is_low() {
            triac.signal(TriacAction::Toggle);
        }

        Timer::after_millis(20).await;
    }
}

/// Initializes embassy for NRF52.
fn init_embassy() -> Peripherals {
    let mut config = embassy_nrf::config::Config::default();

    // Softdevice implicitly utilizes the highest-level interrupt priority
    // We have to move all other interrupts to lower priority, unless
    // random issues and asserts from the Softdevice may (and will) occur

    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;

    embassy_nrf::init(config)
}

/// Initializes SoftDevice
fn init_softdevice() -> &'static mut Softdevice {
    let config = nrf_softdevice::Config {
        // The board is lacking LF crystal because of the space restrictions.
        // Switch to internal RC source for now
        clock: Some(nrf_defines::nrf_clock_lf_cfg_t {
            source: nrf_defines::NRF_CLOCK_LF_SRC_RC as u8,
            rc_ctiv: 16,
            rc_temp_ctiv: 2,
            accuracy: nrf_defines::NRF_CLOCK_LF_ACCURACY_500_PPM as u8,
        }),
        ..Default::default()
    };

    Softdevice::enable(&config)
}

// Main task
#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let p = init_embassy();
    let r = split_resources!(p);

    let softdevice = init_softdevice();
    static TRIAC_SIGNAL: TriacSignal = TriacSignal::new();

    unwrap!(spawner.spawn(bluetooth_task_run(softdevice, &TRIAC_SIGNAL)));
    unwrap!(spawner.spawn(triac_controller_run(r.triac, &TRIAC_SIGNAL)));
    unwrap!(spawner.spawn(button_listener_run(r.button, &TRIAC_SIGNAL)));
}
