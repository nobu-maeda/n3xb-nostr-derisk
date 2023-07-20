use futures::executor::block_on;
use nostr_sdk::prelude::*;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use std::string::{String, ToString};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use strum_macros::Display;

const APP_SPEC_KIND_SUFFIX: u16 = 30078;
const APP_SPEC_D_TAG: &str = "n3x";

type ArcClient = Arc<Mutex<Client>>;
type ArcMutex = Arc<Mutex<i32>>;

#[tokio::main]
async fn main() {
    let my_keys: Keys = Keys::generate();
    let mutex = Arc::new(Mutex::new(0));
    let client = init_nostr_client(&my_keys).await;

    let listen_client = init_nostr_client(&my_keys).await;
    start_notification_listening(&listen_client).await;

    let thread_mutex = mutex.clone();
    let thread_listen_client = listen_client.clone();
    let thread_post_client: Arc<Mutex<Client>> = client.clone();

    _ = thread::spawn(move || {
        block_on(perform_notification_handling(
            &thread_mutex,
            &thread_listen_client,
            &thread_post_client,
        ));
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

        {
            let mut lock = mutex.lock().unwrap();
            *lock = 0;
            match user_input.as_str() {
                "1" => create_offer(&client).await,
                "2" => show_offers(&client).await,
                "3" => take_offer(&client).await,
                _ => println!("Invalid input. Please input a number."),
            }
        }
        println!("");
    }
}

// Common Util

async fn init_nostr_client(keys: &Keys) -> ArcClient {
    let opts = Options::new()
        .wait_for_connection(true)
        .wait_for_send(true)
        .difficulty(8);
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
    custom_tag_filter.insert(
        "#d".to_string(),
        Value::Array(vec![Value::String(APP_SPEC_D_TAG.to_string())]),
    );

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

async fn perform_notification_handling(
    mutex: &ArcMutex,
    listen_client: &ArcClient,
    _post_client: &ArcClient,
) {
    let unlocked_client = listen_client.lock().unwrap();
    unlocked_client
        .handle_notifications(|notification| async {
            let mut lock = mutex.lock().unwrap();

            match notification {
                RelayPoolNotification::Event(url, event) => {
                    println!();
                    println!("Got Event Notification");
                    println!("URL: {}", url.as_str());
                    println!("Event: {}", event.as_json());
                    println!();

                    process_event_message(&event, &unlocked_client).await;
                }
                RelayPoolNotification::Message(_, _) => {
                    // println!();
                    // println!("Got Message Notification");
                    // println!("URL: {}", url.as_str());
                    // println!("Message: {}", message.as_json());
                    // println!();
                }
                RelayPoolNotification::Shutdown => {
                    println!();
                    println!("Got Shutdown Notification");
                    println!();
                }
            };
            *lock = 1;
            Ok(())
        })
        .await
        .unwrap();
}

async fn process_event_message(event: &Event, client: &Client) {
    if let Kind::EncryptedDirectMessage = event.kind {
        process_direct_message(event, &client).await;
    }
}

async fn process_direct_message(event: &Event, client: &Client) {
    if let Ok(msg) = decrypt(
        &client.keys().secret_key().unwrap(),
        &event.pubkey,
        &event.content,
    ) {
        println!("New DM: {}", msg);
        let header = &msg[0..10];
        if header == "n3x_header" {
            process_n3x_direct_message(&msg[10..], &client).await;
        }
    } else {
        println!("Failed to decrypt direct message");
    }
}

async fn process_n3x_direct_message(content: &str, _client: &Client) {
    let n3x_message: N3xMessage = serde_json::from_str(content).unwrap();
    match n3x_message {
        N3xMessage::Order(order_content) => process_n3x_order(&order_content).await,
        N3xMessage::Confirm(confirm_content) => process_n3x_confirmation(&confirm_content).await,
    }
}

async fn process_n3x_order(content: &OrderContent) {
    println!("{:?}", content);
}

async fn process_n3x_confirmation(content: &ConfirmContent) {
    println!("{:?}", content);
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
    price: i64,    // in sats / dollar
}

fn buy_sell_string_to_enum(user_input: String) -> Option<OfferDirection> {
    let user_input_lowercase = user_input.to_lowercase();
    match user_input_lowercase.as_str() {
        "buy" | "buying" => Some(OfferDirection::Buy),
        "sell" | "selling" => Some(OfferDirection::Sell),
        _ => None,
    }
}

fn get_offer_dir() -> OfferDirection {
    loop {
        let user_input = get_user_input();
        if let Some(offer_dir) = buy_sell_string_to_enum(user_input) {
            return offer_dir;
        } else {
            println!(
                "Unrecognized input. Please specify either 'Buy', 'Buying', 'Sell', or 'Selling'"
            );
        }
    }
}

fn get_quantity() -> u64 {
    loop {
        let user_input = get_user_input();
        if let Ok(quantity) = user_input.parse() {
            return quantity;
        } else {
            println!("Unrecognized input. Please input a positive integer");
        }
    }
}

fn get_price() -> i64 {
    loop {
        let user_input = get_user_input();
        if let Ok(price) = user_input.parse() {
            return price;
        } else {
            println!("Unrecognized input. Please input an integer");
        }
    }
}

async fn send_offer(offer_dir: OfferDirection, quantity: u64, price: i64, client: &ArcClient) {
    let app_tag: Tag = Tag::Generic(
        TagKind::Custom("d".to_string()),
        vec![APP_SPEC_D_TAG.to_string()],
    );
    let dir_tag: Tag = Tag::Generic(
        TagKind::Custom("x".to_string()),
        vec![offer_dir.to_string()],
    );
    let event_tags = [app_tag, dir_tag];

    let offer_content = OfferContent { quantity, price };
    let offer_content_string = serde_json::to_string(&offer_content).unwrap();

    let my_keys = client.lock().unwrap().keys();
    let builder = EventBuilder::new(
        Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX),
        offer_content_string,
        &event_tags,
    );
    client
        .lock()
        .unwrap()
        .send_event(builder.to_event(&my_keys).unwrap())
        .await
        .unwrap();
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
    custom_tag_filter.insert(
        "#d".to_string(),
        Value::Array(vec![Value::String(APP_SPEC_D_TAG.to_string())]),
    );
    custom_tag_filter.insert(
        "#x".to_string(),
        Value::Array(vec![Value::String(offer_dir.to_string())]),
    );

    let subscription = Filter::new()
        .kind(Kind::ParameterizedReplaceable(APP_SPEC_KIND_SUFFIX))
        .custom(custom_tag_filter);

    let timeout = Duration::from_secs(1);
    let events = client
        .lock()
        .unwrap()
        .get_events_of(vec![subscription], Some(timeout))
        .await
        .unwrap();

    println!(
        "{} n3xB {} offers found",
        events.len(),
        offer_dir.to_string()
    );
    for event in events {
        println!("{:?}", event.as_json());
    }
}

// Take Offer (Send Order)

#[derive(Debug, Deserialize, Serialize)]
enum N3xMessage {
    Order(OrderContent),
    Confirm(ConfirmContent),
}

#[derive(Debug, Deserialize, Serialize)]
struct OrderContent {
    offer_id: String,
    quantity: u64, // in sats
    price: i64,    // in sats / dollar
}

#[derive(Debug, Deserialize, Serialize)]
struct ConfirmContent {
    order_id: String,
    status_code: String,
}

async fn send_order(
    pubkey_string: String,
    offer_id: String,
    quantity: u64,
    price: i64,
    client: &ArcClient,
) {
    let pubkey = XOnlyPublicKey::from_str(pubkey_string.as_str()).unwrap();

    let order_content = OrderContent {
        offer_id,
        quantity,
        price,
    };
    let n3x_message = N3xMessage::Order(order_content);
    let order_content_string = serde_json::to_string(&n3x_message).unwrap();
    let json_string = "n3x_header".to_owned() + order_content_string.as_str();
    client
        .lock()
        .unwrap()
        .send_direct_msg(pubkey, json_string)
        .await
        .unwrap();
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
