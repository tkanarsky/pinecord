use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{error::Error, time::Duration};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, Characteristic};
use btleplug::platform::{Manager, Peripheral, Adapter};
use discord_rich_presence::{DiscordIpcClient, activity, DiscordIpc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;

const IRON_SERVICE_UUID: Uuid = Uuid::from_u128(0x9eae1000_9d0d_48c5_aa55_33e27f9bc533);

// uuids for reading state (live service)
const UUID_LIVE_TEMP: Uuid = Uuid::from_u128(0xd85ef001_168e_4a71_aa55_33e27f9bc533);
const UUID_SETPOINT: Uuid = Uuid::from_u128(0xd85ef002_168e_4a71_aa55_33e27f9bc533);
const UUID_DC_IN: Uuid = Uuid::from_u128(0xd85ef003_168e_4a71_aa55_33e27f9bc533);
const UUID_HANDLE: Uuid = Uuid::from_u128(0xd85ef004_168e_4a71_aa55_33e27f9bc533);
const UUID_POWER: Uuid = Uuid::from_u128(0xd85ef005_168e_4a71_aa55_33e27f9bc533);
const UUID_POWER_SRC: Uuid = Uuid::from_u128(0xd85ef006_168e_4a71_aa55_33e27f9bc533);
const UUID_TIP_RES: Uuid = Uuid::from_u128(0xd85ef007_168e_4a71_aa55_33e27f9bc533);
const UUID_UPTIME: Uuid = Uuid::from_u128(0xd85ef008_168e_4a71_aa55_33e27f9bc533);
const UUID_MOVEMENT: Uuid = Uuid::from_u128(0xd85ef009_168e_4a71_aa55_33e27f9bc533);
const UUID_MAX_TEMP: Uuid = Uuid::from_u128(0xd85ef00a_168e_4a71_aa55_33e27f9bc533);
const UUID_RAW_TIP: Uuid = Uuid::from_u128(0xd85ef00b_168e_4a71_aa55_33e27f9bc533);
const UUID_HALL: Uuid = Uuid::from_u128(0xd85ef00c_168e_4a71_aa55_33e27f9bc533);
const UUID_OP_MODE: Uuid = Uuid::from_u128(0xd85ef00d_168e_4a71_aa55_33e27f9bc533);
const UUID_WATTS: Uuid = Uuid::from_u128(0xd85ef00e_168e_4a71_aa55_33e27f9bc533);


#[derive(Default, Debug, Serialize, Deserialize)]
pub struct IronStreamingState {
    pub live_temp: u32,
    pub setpoint_temp: u32,
    pub dc_input_voltage: u32,
    pub handle_temp: u32,
    pub power_level: u32,
    pub power_source: u32,
    pub tip_resistance: u32,
    pub uptime: u32,
    pub last_movement: u32,
    pub max_temp: u32,
    pub raw_tip_reading: u32,
    pub hall_sensor: u32,
    pub operating_mode: u32,
    pub estimated_watts: u32
}

async fn read_char_by_uuid<I>(iron: &Peripheral, chars: &I, uuid: Uuid) -> Result<u32, Box<dyn Error>>
    where I: IntoIterator<Item = Characteristic> + Clone
{
    let c = chars.clone().into_iter().find(|c| c.uuid == uuid).ok_or("couldn't find characteristic")?;
    let data = iron.read(&c).await?;
    Ok(u32::from_le_bytes(data[..4].try_into()?))
}

async fn read_iron_state(iron: &Peripheral) -> Result<IronStreamingState, Box<dyn Error>> {
    let chars = iron.characteristics();
    let mut state = IronStreamingState::default();
    state.live_temp = read_char_by_uuid(iron, &chars, UUID_LIVE_TEMP).await?;
    state.setpoint_temp = read_char_by_uuid(iron, &chars, UUID_SETPOINT).await?;
    state.dc_input_voltage = read_char_by_uuid(iron, &chars, UUID_DC_IN).await?;
    state.handle_temp = read_char_by_uuid(iron, &chars, UUID_HANDLE).await?;
    state.power_level = read_char_by_uuid(iron, &chars, UUID_POWER).await?;
    state.power_source = read_char_by_uuid(iron, &chars, UUID_POWER_SRC).await?;
    state.tip_resistance = read_char_by_uuid(iron, &chars, UUID_TIP_RES).await?;
    state.uptime = read_char_by_uuid(iron, &chars, UUID_UPTIME).await?;
    state.last_movement = read_char_by_uuid(iron, &chars, UUID_MOVEMENT).await?;
    state.max_temp = read_char_by_uuid(iron, &chars, UUID_MAX_TEMP).await?;
    state.raw_tip_reading = read_char_by_uuid(iron, &chars, UUID_RAW_TIP).await?;
    state.hall_sensor = read_char_by_uuid(iron, &chars, UUID_HALL).await?;
    state.operating_mode = read_char_by_uuid(iron, &chars, UUID_OP_MODE).await?;
    state.estimated_watts = read_char_by_uuid(iron, &chars, UUID_WATTS).await?;
    Ok(state)
}

type ShutdownSignal = Arc<AtomicBool>;

async fn find_iron(adapter: &Adapter) -> Result<Peripheral, Box<dyn Error>> {
    loop {
        for p in adapter.peripherals().await? {
            if let Some(properties) = p.properties().await? {
                if properties.services.contains(&IRON_SERVICE_UUID) {
                    return Ok(p);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn connect_to_iron(adapter: &Adapter, signal: &ShutdownSignal) -> Result<Peripheral, Box<dyn Error>> {
    println!("scanning for iron...");
    adapter.start_scan(ScanFilter {services: vec![IRON_SERVICE_UUID]}).await?;
    let iron: Peripheral = find_iron(&adapter).await?;
    println!("found iron! connecting...");
    adapter.stop_scan().await?;
    // the connect() method may time out if there is a warm scan result + iron is offline
    // so we repeat until we connect
    while !signal.load(Ordering::Relaxed) {
        if let Ok(connect_res) = tokio::time::timeout (
            Duration::from_secs(5),
            iron.connect(),
        ).await {
            if let Ok(_) = connect_res {
                println!("connected!");
                break;
            }
        }
        // allow bluetooth stack to settle
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    iron.discover_services().await?;
    Ok(iron)
}

async fn run(signal: ShutdownSignal) -> Result<(), Box<dyn Error>> {
    while !signal.load(Ordering::Relaxed) {
        let mut discord_rpc = DiscordIpcClient::new("1320065680883712070")?;
        discord_rpc.connect()?;
        // Reset bluetooth stack between tries
        let manager = Manager::new().await?;
        let adapter = manager.adapters().await?.into_iter().next().ok_or("no adapter")?;        
        let iron = connect_to_iron(&adapter, &signal).await?;
        let mut success = true;
        while !signal.load(Ordering::Relaxed) {
            match read_iron_state(&iron).await {
                Ok(state) => {
                    // let State = format!("°C (~{} W)", state.live_temp, state.estimated_watts);
                    dbg!(&state);
                    let seconds_idle = Duration::from_millis(100 * 
                        (state.uptime.saturating_sub(state.last_movement)) as u64)
                        .as_secs();
                    
                    let temp_diff: i64 = state.setpoint_temp as i64 - state.live_temp as i64;

                    let bottom_line = format!("{} V DC · {} W", 
                        state.dc_input_voltage / 10,
                        state.power_level / 10);

                    let mut top_line: String = String::from("");

                    if state.operating_mode == 0 {
                        top_line = format!("{}°C (in menu)", state.live_temp);
                    } else if state.operating_mode == 1 {
                        if temp_diff < 0 {
                            top_line = format!("{}°C (cooling to {}°C)", state.live_temp, state.setpoint_temp);
                        }  else if temp_diff < 15 {
                            top_line = format!("Soldering at {}°C", state.live_temp);
                        } else {
                            top_line = format!("{}°C (heating to {}°C)", state.live_temp, state.setpoint_temp);
                        }
                    } else if state.operating_mode == 2 {
                        top_line = format!("{}°C (TURBO BOOST ENGAGED)", state.live_temp);
                    } else if state.operating_mode == 3 {
                        if state.live_temp < 50 {
                            top_line = format!("{}°C (idle, resumes to {}°C)", state.live_temp, state.setpoint_temp);
                        } else {
                            top_line = format!("{}°C (cooling to idle temp)", state.live_temp)
                        }
                    }

                    discord_rpc.set_activity(
                        activity::Activity::new()
                            .state(&bottom_line)
                            .details(&top_line)
                    )?; {

                    }
                },
                Err(e) => {
                    println!("lost connection: {}", e);
                    success = false;
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        if success {
            println!("trying to disconnect");
            iron.disconnect().await?;
            discord_rpc.close()?;
        }
    }
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let signal = Arc::new(AtomicBool::new(false));
    let our_signal_handle = signal.clone();
    ctrlc::set_handler(move || {
        our_signal_handle.store(true, Ordering::Relaxed);
    }).expect("failed to set C-c handler");
    run(signal).await
}