use futures::executor::block_on;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::string::{String,ToString};
use std::sync::{Arc,Mutex};
use std::thread;
use std::time::Duration;
use strum_macros::Display;
use nostr_sdk::prelude::*;

const APP_SPEC_KIND_SUFFIX: u16 = 30078;
const APP_SPEC_D_TAG: &str = "n3x";

type ArcClient = Arc<Mutex<Client>>;

#[tokio::main]
async fn main() {
    let my_keys: Keys = Keys::generate();
    let listen_client = init_nostr_client(&my_keys).await;
    let client = init_nostr_client(&my_keys).await;
    start_notification_listening(&listen_client).await;

    let thread_client = listen_client.clone();
    let handle = thread::spawn(move || {
        block_on(perform_notification_handling(&thread_client));
    });

    println!("n3x Nostr Derisk CLI");

    // listen and process subscriptions

     loop {
        println!("=> Options");
        println!("  1. Create New Offer");
        println!("  2. Show Outstanding Offers");
        println!("  3. Take Offer");
        println!("  4. Respond to Order");

        let user_input = get_user_input();

        match user_input.as_str() {
            "1" => create_offer(&client).await,
            "2" => show_offers(&client).await,
            "3" => take_offer(&client).await,
            _ => println!("Invalid input. Please input a number."),
        }

        println!("");
    }
    handle.join().unwrap();
}

// Common Util

async fn init_nostr_client(keys: &Keys) -> ArcClient {
    let opts = Options::new().wait_for_connection(true).wait_for_send(true).difficulty(8);
    let client = Client::new_with_opts(&keys, opts);

    client.add_relay("ws://localhost:8008", None).await.unwrap();
    client.connect().await;
    Arc::new(Mutex::new(client))
}

fn get_user_input() -> String {
    let mut input = String::new();
    _ = std::io::stdin().read_line(&mut input).unwrap();
    println!("");

    input.truncate(input.len() - 1);
    input
}

// Notifications

async fn start_notification_listening(client: &ArcClient) {
    // Offer subscription
    let mut custom_tag_filter = Map::new();
    custom_tag_filter.insert("#d".to_string(), Value::Array(vec![Value::String(APP_SPEC_D_TAG.to_string())]));

    let offer_subscription = Filter::new()
        .since(Timestamp::now())
        .kind(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX))
        .custom(custom_tag_filter);

    // Order subscription
    let order_subscription = Filter::new()
    .since(Timestamp::now())
    .pubkey(client.lock().unwrap().keys().public_key());

    // Confirmation subscription

    let filters = vec![offer_subscription, order_subscription];
    client.lock().unwrap().subscribe(filters).await;
}


async fn perform_notification_handling(client: &ArcClient) {
    client.lock().unwrap().handle_notifications(|notification| {
        match notification {
            RelayPoolNotification::Event(url, event) => {
                println!("Got Event Notification");
                println!("URL: {}", url.as_str());
                println!("Event: {}", event.as_json());
                println!();
            },
            RelayPoolNotification::Message(_, _) => {
                // println!("Got Message Notification");
                // println!("URL: {}", url.as_str());
                // println!("Message: {}", message.as_json());
                // println!();
            },
            RelayPoolNotification::Shutdown => {
                println!("Got Shutdown Notification");
                println!();
            },
        };
        Ok(())
    }).await.unwrap();
}

// Create Offer

#[derive(Display, Debug)]
enum OfferDirection {
    Buy,
    Sell,
}

#[derive(Debug, Deserialize, Serialize)]
struct OfferContent {
    quantity: u64, // in sats
    price: i64, // in sats / dollar
}

fn buy_sell_string_to_enum(user_input: String) -> Option<OfferDirection> {
    let user_input_lowercase = user_input.to_lowercase();
    match user_input_lowercase.as_str() {
        "buy" | "buying" => Some(OfferDirection::Buy),
        "sell" | "selling" => Some(OfferDirection::Sell),
        _ => None
    }
}

fn get_offer_dir() -> OfferDirection {
    loop {
        let user_input = get_user_input();
        if let Some(offer_dir) = buy_sell_string_to_enum(user_input) {
            return offer_dir
        } else {
            println!("Unrecognized input. Please specify either 'Buy', 'Buying', 'Sell', or 'Selling'");
        }
    }
}

fn get_quantity() -> u64 {
    loop {
        let user_input = get_user_input();
        if let Ok(quantity) = user_input.parse() {
            return quantity
        } else {
            println!("Unrecognized input. Please input a positive integer");
        }
    }
}

fn get_price() -> i64 {
    loop {
        let user_input = get_user_input();
        if let Ok(price) = user_input.parse() {
            return price
        } else {
            println!("Unrecognized input. Please input an integer");
        }
    }
}

async fn send_offer(offer_dir: OfferDirection, quantity: u64, price: i64, client: &ArcClient) {
    let app_tag: Tag = Tag::Generic(TagKind::Custom("d".to_string()), vec![APP_SPEC_D_TAG.to_string()]);
    let dir_tag: Tag = Tag::Generic(TagKind::Custom("x".to_string()), vec![offer_dir.to_string()]);
    let event_tags = [app_tag, dir_tag];

    let offer_content = OfferContent { quantity, price };
    let offer_content_string = serde_json::to_string(&offer_content).unwrap();

    let my_keys = client.lock().unwrap().keys();
    let builder = EventBuilder::new(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX), offer_content_string, &event_tags);
    client.lock().unwrap().send_event(builder.to_event(&my_keys).unwrap()).await.unwrap();
}

async fn create_offer(client: &ArcClient) {
    println!("Are you Buying or Selling?");
    let offer_dir = get_offer_dir();
    println!("How many Sats?");
    let quantity = get_quantity();
    println!("At what price?");
    let price = get_price();
    send_offer(offer_dir, quantity, price, client).await;
}

// Show Outstanding Offers

async fn show_offers(client: &ArcClient) {
    println!("Are you trying to see Buy Offers, or Sell Offers?");
    let offer_dir = get_offer_dir();

    // Kind, d = n3xB, offerDirection
    let mut custom_tag_filter = Map::new();
    custom_tag_filter.insert("#d".to_string(), Value::Array(vec![Value::String(APP_SPEC_D_TAG.to_string())]));
    custom_tag_filter.insert("#x".to_string(), Value::Array(vec![Value::String(offer_dir.to_string())]));

    let subscription = Filter::new()
         .kind(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX))
         .custom(custom_tag_filter);
    
     let timeout = Duration::from_secs(1);
     let events = client.lock().unwrap()
         .get_events_of(vec![subscription], Some(timeout))
         .await.unwrap();

    println!("{} n3xB {} offers found", events.len(), offer_dir.to_string());
    for event in events {
        println!("{:?}", event.as_json());
    }
}

// Take Offer (Send Order)

#[derive(Debug, Deserialize, Serialize)]
struct OrderContent {
    offer_id: String,
    quantity: u64, // in sats
    price: i64, // in sats / dollar
}

async fn send_order(pubkey_string: String, offer_id: String, quantity: u64, price: i64, client: &ArcClient) {
    let pubkey = XOnlyPublicKey::from_str(pubkey_string.as_str()).unwrap();

    let order_content = OrderContent { offer_id, quantity, price };
    let order_content_string = serde_json::to_string(&order_content).unwrap();

    client.lock().unwrap().send_direct_msg(pubkey, order_content_string).await.unwrap();
}

async fn take_offer(client: &ArcClient) {
    println!("What Offer ID?");
    let offer_id = get_user_input();
    println!("What Pubkey?");
    let pubkey = get_user_input();
    println!("How many Sats?");
    let quantity = get_quantity();
    println!("At what price?");
    let price = get_price();
    send_order(pubkey, offer_id, quantity, price, client).await;
}

// Respond to Order

// async fn respond_to_order(client: &Client) {
    // pubkey
    // order-id
    // Yes or No (with reason, basically if the Offer existed)
    // send DM
// }