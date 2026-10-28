// Import necessary AMQP options, types, and connection utilities from the lapin crate
use lapin::{
    options::{ExchangeDeclareOptions, QueueBindOptions, QueueDeclareOptions},
    types::{AMQPValue, FieldTable, ShortString},
    Connection, ConnectionProperties, ExchangeKind,
};
// Import standard error trait for handling asynchronous main function errors
use std::error::Error;

// Mark the main function as an asynchronous entry point managed by the Tokio runtime
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Retrieve the RabbitMQ connection string from environment variables, defaulting to local AMQP server URI
    let addr =
        std::env::var("AMQP_ADDR").unwrap_or_else(|_| "amqp://127.0.0.1:5672/%2f".to_string());

    // Establish an asynchronous TCP connection to the RabbitMQ broker using default properties
    let connection = Connection::connect(&addr, ConnectionProperties::default()).await?;

    // Open a lightweight virtual communication channel over the established connection
    let channel = connection.create_channel().await?;

    // Define the string identifier for the primary processing exchange
    let main_exchange = "chainflow.retry.exchange";
    // Define the string identifier for the delayed retry queue's exchange
    let retry_exchange = "chainflow.retry.delay.exchange";
    // Define the string identifier for the dead-letter exchange (DLX) for permanently failed messages
    let dlx_exchange = "chainflow.retry.dlx";

    // Iterate through all three exchange names to declare them on the RabbitMQ server
    for exchange in [main_exchange, retry_exchange, dlx_exchange] {
        // Send an exchange declaration command using Direct routing semantics and default options
        channel
            .exchange_declare(
                exchange.into(),      // Convert string slice to AMQP ShortString/String type
                ExchangeKind::Direct, // Use direct exchange type for exact routing key matching
                ExchangeDeclareOptions::default(), // Use default exchange declaration options (non-durable/durable as needed)
                FieldTable::default(),             // No custom arguments needed for basic exchanges
            )
            .await?;
    }

    // Initialize argument table for configuring the main processing queue's dead-letter behavior
    let mut main_args = FieldTable::default();
    // Configure DLX so rejected/nacked messages are automatically routed to the retry exchange
    main_args.insert(
        ShortString::from("x-dead-letter-exchange"),
        AMQPValue::LongString(retry_exchange.into()),
    );
    // Specify the routing key that should be used when forwarding dead-lettered messages to the retry exchange
    main_args.insert(
        ShortString::from("x-dead-letter-routing-key"),
        AMQPValue::LongString("retry".into()),
    );

    // Declare the durable main task queue with the dead-letter arguments attached
    channel
        .queue_declare(
            "chainflow.retry.main".into(),  // Name of the main processing queue
            QueueDeclareOptions::durable(), // Mark queue as durable so it survives broker restarts
            main_args,                      // Attach the dead-letter configuration table
        )
        .await?;

    // Initialize argument table for configuring the delay queue's TTL and redirection behavior
    let mut retry_args = FieldTable::default();
    // Set a Message Time-To-Live (TTL) of 5000 milliseconds (5 seconds) for messages resting in the delay queue
    retry_args.insert(ShortString::from("x-message-ttl"), AMQPValue::LongInt(5000));
    // Configure the delay queue to dead-letter expired messages back to the main processing exchange
    retry_args.insert(
        ShortString::from("x-dead-letter-exchange"),
        AMQPValue::LongString(main_exchange.into()),
    );
    // Specify the routing key to use when sending expired messages back into the main workflow
    retry_args.insert(
        ShortString::from("x-dead-letter-routing-key"),
        AMQPValue::LongString("process".into()),
    );

    // Declare the durable delay/retry queue using the 5-second TTL and fallback routing rules
    channel
        .queue_declare(
            "chainflow.retry.delay".into(), // Name of the delay/waiting queue
            QueueDeclareOptions::durable(), // Mark queue as durable
            retry_args,                     // Attach TTL and recycling configuration arguments
        )
        .await?;

    // Declare the final dead-letter queue (DLQ) for trapping permanently unprocessable messages
    channel
        .queue_declare(
            "chainflow.retry.dead".into(),  // Name of the dead-letter queue
            QueueDeclareOptions::durable(), // Mark queue as durable
            FieldTable::default(),          // No custom arguments required for the final sink
        )
        .await?;

    // Bind the main processing queue to the main exchange using the routing key "process"
    channel
        .queue_bind(
            "chainflow.retry.main".into(), // Target queue to bind
            main_exchange.into(),          // Source exchange to bind against
            "process".into(),              // Routing key filter
            QueueBindOptions::default(),   // Use default binding options
            FieldTable::default(),         // No extra binding arguments
        )
        .await?;

    // Bind the delay queue to the retry exchange using the routing key "retry"
    channel
        .queue_bind(
            "chainflow.retry.delay".into(), // Target delay queue to bind
            retry_exchange.into(),          // Source retry exchange
            "retry".into(),                 // Routing key filter
            QueueBindOptions::default(),    // Use default binding options
            FieldTable::default(),          // No extra binding arguments
        )
        .await?;

    // Bind the dead-letter queue to the DLX exchange using the routing key "dead"
    channel
        .queue_bind(
            "chainflow.retry.dead".into(), // Target dead-letter queue to bind
            dlx_exchange.into(),           // Source DLX exchange
            "dead".into(),                 // Routing key filter
            QueueBindOptions::default(),   // Use default binding options
            FieldTable::default(),         // No extra binding arguments
        )
        .await?;

    // Print a success confirmation message once the full RabbitMQ topology has been set up
    println!("Main, retry, and dead-letter queues configured");

    // Return Ok to indicate successful execution of the main async function
    Ok(())
}
