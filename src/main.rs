use rand::Rng;
use tokio;
use std::time::Duration;

use teloxide::{
    prelude2::*,
    types::{Update, User, ChatPermissions, True},
};

use std::sync::Arc;
use dashmap::DashMap;

#[tokio::main]
async fn main() {
    teloxide::enable_logging!();
    log::info!("Starting captcha bot...");

    let hashmap: Arc<DashMap<(i64, i64), (i64, i32)>> = Arc::new(DashMap::new());
    let bot = Bot::from_env().auto_send();
    const NSECS: u64 = 60;

    let handler = Update::filter_message()
        .branch(
            Message::filter_new_chat_members()
                .endpoint(|msg: Message, chatmemb: Vec<User>, bot: AutoSend<Bot>, hashmap: Arc<DashMap<(i64, i64), (i64, i32)>>| async move {
                    for user in chatmemb {
                        log::info!("New user: {:?}", user);
                        let num1 = rand::thread_rng().gen_range(1..20);
                        let num2 = rand::thread_rng().gen_range(1..20);
                        let text = std::format!("Привет, {}! Для проверки, что Вы не бот решите следующий пример: {} плюс {}, у вас на это {} секунд",
                                             user.first_name,
                                            num1,
                                            num2,
                                            NSECS);
                        let bot_msg = bot.send_message(msg.chat.id, text)
                           .reply_to_message_id(msg.id)
                           .await
                           .expect("Failed to send message");
                        let bot_clone = bot.clone();
                        let chat_id = msg.chat.id;

                        let user_id = user.id;
                        hashmap.insert((chat_id, user_id), (num1 + num2, bot_msg.id));
                        let hashmap2 = hashmap.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_secs(NSECS)).await;
                            log::info!("{} seconds elapsed!", NSECS);
                            if let Some(answer) = hashmap2.get(&(msg.chat.id, user.id)) {
                                let bot_msg_id = (*answer).1;
                                bot_clone.delete_message(chat_id, bot_msg_id)
                                    .await
                                    .unwrap_or_else(|_| { log::error!("Failed to delete message!"); True });
                                drop(answer);
                                hashmap2.remove(&(chat_id, user_id));
                                bot_clone.restrict_chat_member(chat_id, user_id, ChatPermissions::empty())
                                    .await
                                    .unwrap_or_else(|_| { log::error!("Failed to restrict user!"); True });
                                bot_clone.ban_chat_member(chat_id, user_id)
                                    .await
                                    .unwrap_or_else(|_| { log::error!("Failed to ban user!"); True });
                                }
                            });
                        }
                        respond(())
                    }),
            )
            .branch(
                // Filter a maintainer by a used ID.
                dptree::filter(|msg: Message| {
                    msg.chat.is_group() || msg.chat.is_supergroup()
                })
                .endpoint(
                    |msg: Message, bot: AutoSend<Bot>, hashmap: Arc<DashMap<(i64, i64), (i64, i32)>>| async move {
                        if let Some(user) = msg.from() {
                            if let Some(answer) = hashmap.get(&(msg.chat.id, user.id)) {
                                let attempt: i64 = msg.text()
                                    .unwrap_or("-1")
                                    .parse()
                                    .unwrap_or(-1);
                                if attempt == (*answer).0 {
                                    log::info!("Got correct answer from user {}", user.id);
                                    let msg_id = (*answer).1;
                                    drop(answer);
                                    hashmap.remove(&(msg.chat.id, user.id));
                                    bot.delete_message(msg.chat.id, msg_id)
                                        .await
                                        .unwrap_or_else(|_| { log::error!("Failed to delete message!"); True });
                                    bot.delete_message(msg.chat.id, msg.id)
                                        .await
                                        .unwrap_or_else(|_| { log::error!("Failed to delete message!"); True });
                                } else {
                                    bot.delete_message(msg.chat.id, msg.id)
                                        .await
                                        .unwrap_or_else(|_| { log::error!("Failed to delete message!"); True });
                                }
                            }
                        }
                        respond(())
                    },
                ),
            );

        Dispatcher::builder(bot, handler)
            // Here you specify initial dependencies that all handlers will receive; they can be
            // database connections, configurations, and other auxiliary arguments. It is similar to
            // `actix_web::Extensions`.
            .dependencies(dptree::deps![hashmap])
            // If no handler succeeded to handle an update, this closure will be called.
            .default_handler(|upd| async move {
                log::warn!("Unhandled update: {:?}", upd);
            })
            // If the dispatcher fails for some reason, execute this handler.
            .error_handler(LoggingErrorHandler::with_custom_text(
                "An error has occurred in the dispatcher",
            ))
            .build()
            .setup_ctrlc_handler()
            .dispatch()
            .await;
} // One long main :)
