use serde_json::to_string;
use sysinfo::{ComponentExt, DiskExt, NetworkExt, NetworksExt, ProcessorExt, System, SystemExt};
use crate::models::{AllData, ComponentData, Disk, SystemData, SystemOs, NetworkInfo};
use pnet::datalink;

pub fn get_mac_address() -> String {
    let mut mac_addr = String::new();
    for interface in datalink::interfaces() {
        if interface.name == "enp2s0" || interface.name == "eth0" {
            mac_addr = interface.mac.unwrap().to_string();
        }
    }
    return mac_addr.replace(":", "").to_uppercase()
}

pub fn get_ip_address() -> String {
    let interfaces = datalink::interfaces();
    let mut ip_addr = String::new();
    for interface in interfaces {
        if interface.name == "eth1" || interface.name == "wlp3s0" {
            ip_addr = interface.ips[0].to_string();
        }
    }
    return ip_addr.replace("/24", "")
}

pub fn get_temp(req_sys: &System) -> i32
{
    // For every component, if it's the CPU, put its temperature in variable to return
    let mut wanted_temp: f32 = -1.;
    for comp in req_sys.get_components() { if comp.get_label() == "CPU" { wanted_temp = comp.get_temperature(); } }

    wanted_temp as i32
}

pub fn get_ram_use(req_sys: &System) -> f32
{
    (req_sys.get_used_memory() as f32) / (req_sys.get_total_memory() as f32) * 100.
}


pub fn run_stats() -> String {
    let mut sys = System::new_all();
    sys.refresh_all();

    let system_data = SystemData {
        total_memory: sys.get_total_memory(),
        used_memory: sys.get_used_memory(),
        total_swap: sys.get_total_swap(),
        used_swap: sys.get_used_swap(),
        uptime: sys.get_uptime(),
        boot_time: sys.get_boot_time(),
        cpu_temp: get_temp(&sys),
        cpu_percentage: sys.get_global_processor_info().get_cpu_usage(),
        ram_percentage: get_ram_use(&sys),
    };

    let system_os = SystemOs {
        name: System::get_name(&Default::default()).expect(""),
        kernel_version: System::get_kernel_version(&Default::default()).expect(""),
        os_version: System::get_os_version(&Default::default()).expect(""),
        host_name: System::get_host_name(&Default::default()).expect(""),
        cpus: sys.get_processors().len() as i32,
        processor: sys.get_processors()[0].get_brand().to_string(),
        stats: system_data,
    };

    let mut network_info = Vec::new();
    for interface in datalink::interfaces() {
        let info = NetworkInfo {
            index: interface.index,
            name: interface.name.clone(),
            mac: interface.mac.unwrap().to_string(),
            ip_addr: interface.ips.clone(),
            flags: interface.flags.clone(),
            total_received: sys.get_networks().iter().find(|(interface_name, _)| **interface_name == interface.name).unwrap().1.get_received(),
            total_transmitted: sys.get_networks().iter().find(|(interface_name, _)| **interface_name == interface.name).unwrap().1.get_transmitted(),
        };
        network_info.push(info);
    }

    let all_data = AllData {
        system: system_os,
        network: network_info,
        components: sys.get_components()
            .iter()
            .map(|component| ComponentData {
                name: component.get_label().to_string(),
                temperature: component.get_temperature().to_string(),
            })
            .collect(),
        disk: sys.get_disks()
            .iter()
            .map(|disk| Disk {
                name: disk.get_name().to_string_lossy().to_string(),
                file_system: String::from_utf8_lossy(disk.get_file_system()).to_string(),
                total_space: disk.get_total_space(),
                available_space: disk.get_available_space(),
            })
            .collect(),
    };
    return to_string(&all_data).map_err(|e| e.to_string()).expect("")
}