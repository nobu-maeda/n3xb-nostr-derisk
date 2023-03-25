use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::string::{String,ToString};
use std::time::Duration;
use strum_macros::Display;
use nostr_sdk::prelude::*;

const APP_SPEC_KIND_SUFFIX: u16 = 30078;
const APP_SPEC_D_TAG: &str = "n3x";

#[tokio::main]
async fn main() {
    let client = init_nostr_client().await;

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
            _ => println!("Invalid input. Please input a number."),
        }

        println!("");
    }
}

// Common Util

async fn init_nostr_client() -> Client {
    let my_keys: Keys = Keys::generate();
    let opts = Options::new().wait_for_connection(true).wait_for_send(true).difficulty(8);
    let client = Client::new_with_opts(&my_keys, opts);

    client.add_relay("ws://localhost:8008", None).await.unwrap();
    client.connect().await;
    client
}

fn get_user_input() -> String {
    let mut input = String::new();
    _ = std::io::stdin().read_line(&mut input).unwrap();
    println!("");

    input.truncate(input.len() - 1);
    input
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

async fn send_offer(offer_dir: OfferDirection, quantity: u64, price: i64, client: &Client) {
    let app_tag: Tag = Tag::Generic(TagKind::Custom("d".to_string()), vec![APP_SPEC_D_TAG.to_string()]);
    let dir_tag: Tag = Tag::Generic(TagKind::Custom("x".to_string()), vec![offer_dir.to_string()]);
    let event_tags = [app_tag, dir_tag];

    let offer_content = OfferContent { quantity, price };
    let offer_content_string = serde_json::to_string(&offer_content).unwrap();

    let builder = EventBuilder::new(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX), offer_content_string, &event_tags);
    client.send_event(builder.to_event(&client.keys()).unwrap()).await.unwrap();
}

async fn create_offer(client: &Client) {
    println!("Are you Buying or Selling?");
    let offer_dir = get_offer_dir();
    println!("How many Sats?");
    let quantity = get_quantity();
    println!("At what price?");
    let price = get_price();
    send_offer(offer_dir, quantity, price, client).await;
}

// Show Outstanding Offers

async fn show_offers(client: &Client) {
    println!("Are you trying to see Buy Offers, or Sell Offers?");
    let offer_dir = get_offer_dir();

    // Kind, d = n3xB, offerDirection
    let mut custom_tag_filter = Map::new();
    custom_tag_filter.insert("#d".to_string(), Value::Array(vec![Value::String(APP_SPEC_D_TAG.to_string())]));
    custom_tag_filter.insert("#x".to_string(), Value::Array(vec![Value::String(offer_dir.to_string())]));

    let subscription = Filter::new()
        //  .since(Timestamp::now())
         .kind(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX))
         .custom(custom_tag_filter);
    
     let timeout = Duration::from_secs(1);
     let events = client
         .get_events_of(vec![subscription], Some(timeout))
         .await.unwrap();

    println!("{} n3xB {} offers found", events.len(), offer_dir.to_string());
    for event in events {
        println!("{:?}", event.as_json());
    }
}
