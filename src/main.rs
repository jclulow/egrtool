extern crate reqwest;
extern crate serde;
extern crate serde_json;

use std::process::exit;
use std::io::Result;
use std::io::Error;
use std::io::ErrorKind;
use std::collections::HashMap;

use chrono::prelude::*;

use serde::Deserialize;

struct Context {
    client: reqwest::Client,
    route_names: HashMap<i64, String>,
}

#[derive(Debug, Deserialize)]
struct Arrival {
    #[serde(rename = "RouteID")]
    route_id: i64,
    #[serde(rename = "StopID")]
    stop_id: i64,
    #[serde(rename = "BusName")]
    bus_name: Option<String>,
    #[serde(rename = "RouteName")]
    route_name: String,
    #[serde(rename = "ArriveTime")]
    arrive_time: String,
    #[serde(rename = "SecondsToArrival")]
    eta_seconds: f64,
    #[serde(rename = "SchedulePrediction")]
    just_scheduled: bool,
}

#[derive(Debug, Deserialize)]
struct Arrivals {
    #[serde(rename = "RouteID")]
    route_id: i64,
    #[serde(rename = "Arrivals")]
    arrivals: Vec<Arrival>,
}

#[derive(Debug, Deserialize)]
struct Region {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Route {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "CustomerID")]
    customer_id: i64,
    #[serde(rename = "Name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct Direction {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "RouteID")]
    route_id: i64,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Stops")]
    stops: Vec<Stop>,
}

#[derive(Debug, Deserialize)]
struct Stop {
    #[serde(rename = "ID")]
    id: i64,
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "RtpiNumber")]
    rtpi_number: i64,
}

fn url(p: &str) -> String {
    format!("https://www.egrshuttle.com/{}", p)
}

fn get_regions(c: &Context) -> Result<Vec<Region>> {
    let u = url("Regions");

    let req = c.client.get(&u);

    fn ee(s: String) -> Result<Vec<Region>> {
        Err(Error::new(ErrorKind::Other, s))
    }

    let mut res = match req.send() {
        Err(e) => {
            return ee(format!("request send: {}", e));
        }
        Ok(r) => r
    };

    if res.status() != reqwest::StatusCode::OK {
        return ee(format!("odd response ({}): {:?}", res.status(), res));
    }

    let regions: Vec<Region> = match res.json() {
        Ok(r) => r,
        Err(e) => {
            return ee(format!("JSON parse: {}", e));
        }
    };

    Ok(regions)
}

fn get_routes(c: &Context, region_id: i64) -> Result<Vec<Route>> {
    let u = url(&format!("Region/{}/Routes", region_id));

    println!("GET {}", u);

    let req = c.client.get(&u);

    fn ee(s: String) -> Result<Vec<Route>> {
        Err(Error::new(ErrorKind::Other, s))
    }

    let mut res = match req.send() {
        Err(e) => {
            return ee(format!("request send: {}", e));
        }
        Ok(r) => r
    };

    if res.status() != reqwest::StatusCode::OK {
        return ee(format!("odd response ({}): {:?}", res.status(), res));
    }

    let routes: Vec<Route> = match res.json() {
        Ok(r) => r,
        Err(e) => {
            return ee(format!("JSON parse: {}", e));
        }
    };

    Ok(routes)
}

fn get_directions(c: &Context, route_id: i64) -> Result<Vec<Direction>> {
    let u = url(&format!("Route/{}/Directions", route_id));

    println!("GET {}", u);

    let req = c.client.get(&u);

    fn ee(s: String) -> Result<Vec<Direction>> {
        Err(Error::new(ErrorKind::Other, s))
    }

    let mut res = match req.send() {
        Err(e) => {
            return ee(format!("request send: {}", e));
        }
        Ok(r) => r
    };

    if res.status() != reqwest::StatusCode::OK {
        return ee(format!("odd response ({}): {:?}", res.status(), res));
    }

    let dirs: Vec<Direction> = match res.json() {
        Ok(r) => r,
        Err(e) => {
            return ee(format!("JSON parse: {}", e));
        }
    };

    Ok(dirs)
}

fn get_arrivals(c: &Context, stop_id: i64, customer_id: i64)
    -> Result<Vec<Arrival>>
{
    let u = url(&format!("Stop/{}/Arrivals?customerID={}", stop_id,
        customer_id));

    println!("GET {}", u);

    let req = c.client.get(&u);

    fn ee(s: String) -> Result<Vec<Arrival>> {
        Err(Error::new(ErrorKind::Other, s))
    }

    let mut res = match req.send() {
        Err(e) => {
            return ee(format!("request send: {}", e));
        }
        Ok(r) => r
    };

    if res.status() != reqwest::StatusCode::OK {
        return ee(format!("odd response ({}): {:?}", res.status(), res));
    }

    let arrivals: Vec<Arrivals> = match res.json() {
        Ok(r) => r,
        Err(e) => {
            return ee(format!("JSON parse: {}", e));
        }
    };

    let mut a: Vec<Arrival> = Vec::new();
    for arrival in arrivals {
        for aa in arrival.arrivals {
            a.push(aa);
        }
    }

    a.sort_by(|a, b| {
        a.eta_seconds.partial_cmp(&b.eta_seconds).unwrap()
    });

    Ok(a)
}

fn sleep(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn display_thread() {
    let pole = pd3000::PD3000::open();

    pole.reset();
    pole.mode_normal();
    pole.cursor_hide();

    let times: Vec<f64> = vec![ /*38.1,*/ 449.7, 1337.1, 9999.9 ];

    let mut on = true;

    loop {
        /*
         * Current local time:
         */
        let local: DateTime<Local> = Local::now();
        // let f = local.format("%H:%M:%S");

        /*
         * Build list of departures:
         */
        let mut s = String::new();
        let mut q = String::new();

        for t in &times {
            if s.len() > 0 {
                s.push_str(", ");
            }
            if q.len() > 0 {
                q.push_str("  ");
            }

            let mins = (t / 60.0).floor() as i64;
            //s.push_str(&format!("{}", mins.floor()));

            let when = local.checked_add_signed(chrono::Duration::minutes(mins)).unwrap();
            let f = when.format("%H:%M");

            s.push_str(&format!("{}", f));
            q.push_str(&format!("{:>5}", format!("+{}m", mins)));

            if s.len() > 15 {
                break;
            }

            // let when = local.checked_add_signed(
            //     chrono::Duration::from_std(
            //     std::time::Duration::from_secs_f64(*t)).unwrap()).unwrap();

            // let f = when.format("%H:%M:%S");
            // s.push_str(&format!("{}", f));
        }

        pole.move_to(0, 0);
        pole.writes(&format!("{:>20}", s));
        pole.move_to(0, 1);
        pole.writes(&format!("{:>20}", q));

        pole.move_to(0, 1);
        if on {
            pole.writec('.');
        }
        on = !on;

        sleep(1000);
    }
}

fn main() {
    let cb = reqwest::ClientBuilder::new()
        .redirect(reqwest::RedirectPolicy::none());

    let mut c = Context {
        client: cb.build().unwrap(),
        route_names: HashMap::new(),
    };

    std::thread::spawn(display_thread);

    let r = get_regions(&c).expect("get regions");
    println!("regions: {:#?}", r);
    const REGION: &str = "No Region";
    let ids: Vec<i64> = r.iter().filter(|region| {
        region.name == REGION
    }).map(|region| {
        region.id
    }).collect();
    println!("matching regions: {:#?}", ids);
    if ids.len() < 1 {
        eprintln!("no ID found for region \"{}\"", REGION);
        exit(10);
    }

    let routes = get_routes(&c, ids[0]).expect("get routes");
    println!("routes: {:#?}", routes);

    let route_ids: Vec<i64> = routes.iter().filter(|route| {
        route.name == "Hollis" || route.name == "South Hollis"
    }).map(|route| {
        route.id
    }).collect();
    println!("matching routes: {:#?}", route_ids);

    let mut stop_ids: Vec<i64> = Vec::new();

    for rid in route_ids {
        //println!("ROUTE {} DIRECTIONS:", rid);
        let dirs = get_directions(&c, rid).expect("get directions");

        for dir in dirs {
            c.route_names.insert(dir.route_id, dir.name);

            for stop in dir.stops {
                if stop.rtpi_number == 5323 ||
                    stop.name == "Park Ave @ Pixar (EB)"
                {
                    if !stop_ids.contains(&stop.id) {
                        stop_ids.push(stop.id);
                    }
                }
            }
        }
    }

    println!("STOP IDS: {:#?}", stop_ids);
    println!("ROUTE NAMES: {:#?}", c.route_names);

    loop {
        println!("");
        for stop_id in &stop_ids {
            let arrivals = get_arrivals(&c, *stop_id, 86 /* Customer ID XXX */)
                .expect("get arrivals");

            println!("STOP ID {} ARRIVALS:", *stop_id);

            for a in arrivals {
                let sched = if a.just_scheduled { "SCHEDULED" } else { "ACTUAL" };
                let busname = if let Some(n) = a.bus_name { format!("#{}", n) }
                    else { "-".to_string() };

                println!("{:16} {:8} {:10} {:8}", a.route_name, a.arrive_time,
                    sched, busname);
            }
        }

        sleep(30_000);
    }
}
