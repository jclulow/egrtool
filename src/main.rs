use std::process::exit;
use std::io::Result;
use std::io::Error;
use std::io::ErrorKind;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::Arc;

use chrono::prelude::*;

use serde::Deserialize;

struct RecentArrivals {
    list: Vec<(DateTime<Local>, Arrival)>,
}

struct Context {
    client: reqwest::Client,
    route_names: HashMap<i64, String>,
    recent_arrivals: Arc<Mutex<RecentArrivals>>,
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

struct Arrivals {
    read_time: DateTime<Local>,
    list: Vec<Arrival>,
}

#[derive(Debug, Deserialize)]
struct ResponseArrivals {
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
    -> Result<Arrivals>
{
    let u = url(&format!("Stop/{}/Arrivals?customerID={}", stop_id,
        customer_id));

    println!("GET {}", u);

    let req = c.client.get(&u);

    fn ee(s: String) -> Result<Arrivals> {
        Err(Error::new(ErrorKind::Other, s))
    }

    let mut res = match req.send() {
        Err(e) => {
            return ee(format!("request send: {}", e));
        }
        Ok(r) => r
    };

    let read_time: DateTime<Local> = Local::now();

    if res.status() != reqwest::StatusCode::OK {
        return ee(format!("odd response ({}): {:?}", res.status(), res));
    }

    let arrivals: Vec<ResponseArrivals> = match res.json() {
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

    Ok(Arrivals {
        read_time: read_time,
        list: a,
    })
}

fn sleep(ms: u64) {
    std::thread::sleep(std::time::Duration::from_millis(ms));
}

fn reset_display() {
    let pole = pd3000::PD3000::open();

    pole.reset();
    pole.cursor_hide();
}

fn display_thread(ra: Arc<Mutex<RecentArrivals>>) {
    let pole = pd3000::PD3000::open();

    pole.reset();
    pole.mode_normal();
    pole.cursor_hide();

    let mut on = true;

    loop {
        /*
         * Current local time:
         */
        let local: DateTime<Local> = Local::now();
        // let f = local.format("%H:%M:%S");

        /*
         * Build two display lines: a list of departure times on the first line,
         * and the remaining time until departure (e.g., +2 minutes) on the
         * second line.  It should look roughly like this on the display:
         *     /--------------------\
         *     | 11:12  11:39  11:59|
         *     |. +11m   +38m   +58m|
         *     \--------------------/
         */
        let mut s = String::new();
        let mut q = String::new();

        {
            let times = &ra.lock().unwrap().list;

            for t in times {
                if s.len() > 0 {
                    s.push_str("  ");
                }
                if q.len() > 0 {
                    q.push_str("  ");
                }

                /*
                 * The arrival event is projected to occur "eta_seconds" after
                 * the request was made:
                 */
                let delta = chrono::Duration::seconds(t.1.eta_seconds as i64);

                /*
                 * The absolute projected event time:
                 */
                let when = t.0.checked_add_signed(delta).unwrap();

                /*
                 * Calculate the time remaining between now and the projected
                 * event time:
                 */
                let rem = when.signed_duration_since(local);

                s.push_str(&format!("{}", when.format("%H:%M")));

                let remsec = rem.num_seconds();
                let remstr = if remsec < 0 {
                    "now?".to_string()
                } else if remsec < 60 {
                    format!("+{}s", remsec)
                } else {
                    format!("+{}m", rem.num_minutes())
                };
                q.push_str(&format!("{:>5}", remstr));

                if s.len() > 15 {
                    break;
                }
            }
        }

        pole.move_to(0, 0);
        pole.writes(&format!("{:>20}", s));
        pole.move_to(0, 1);
        pole.writes(&format!("{:>20}", q));

        /*
         * Flash a period in the bottom left corner to show that the display is
         * still being refreshed.
         */
        pole.move_to(0, 1);
        if on {
            pole.writec('.');
        }
        on = !on;

        sleep(1000);
    }
}

fn spawn_display_thread(c: &Context) {
    let ra = Arc::clone(&c.recent_arrivals);
    std::thread::spawn(move || {
        display_thread(ra);
    });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && args[1] == "reset" {
        reset_display();
        exit(0);
    }

    let cb = reqwest::ClientBuilder::new()
        .redirect(reqwest::RedirectPolicy::none());

    let mut c = Context {
        client: cb.build().unwrap(),
        route_names: HashMap::new(),
        recent_arrivals: Arc::new(Mutex::new(RecentArrivals {
            list: Vec::new()
        })),
    };

    spawn_display_thread(&c);

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

        let mut real_arrivals: Vec<(DateTime<Local>, Arrival)> = Vec::new();
        let mut fail = false;

        for stop_id in &stop_ids {
            let arrivals = match get_arrivals(&c, *stop_id,
                86 /* Customer ID XXX */)
            {
                Err(e) => {
                    println!("ERROR: get arrivals: {}", e);
                    fail = true;
                    break;
                }
                Ok(a) => a
            };

            println!("STOP ID {} ARRIVALS:", *stop_id);

            for a in arrivals.list {
                let sched = if a.just_scheduled { "SCHEDULED" } else { "ACTUAL" };
                let busname = if let Some(n) = &a.bus_name { format!("#{}", n) }
                    else { "-".to_string() };

                println!("{:16} {:8} {:10} {:8}", a.route_name, a.arrive_time,
                    sched, busname);

                if !a.just_scheduled {
                    real_arrivals.push((arrivals.read_time, a));
                }
            }
        }

        if fail {
            sleep(5_000);
            continue;
        }

        {
            let ra = &mut *c.recent_arrivals.lock().unwrap();
            ra.list = real_arrivals;
        }

        sleep(30_000);
    }
}
