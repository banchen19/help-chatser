use std::io::{Read, Write};

use actix_files::{Files, NamedFile};
use actix_web::{
    middleware::Logger,
    web::{self, route},
    App, HttpResponse, HttpServer, Responder,
};

pub async fn getconn(url: &str) -> SqliteConnection {
    let conn: SqliteConnection = SqliteConnection::connect(url)
        .await
        .unwrap_or_else(|_| panic!("sqlite connect error"));
    conn
}

use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::authentication::Credentials,
    Address, Message, SmtpTransport, Transport,
};
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, Connection, Row, Sqlite, SqliteConnection};

#[actix_web::main]
async fn main() -> Result<(), std::io::Error> {
    // 初始化html
    let mut file = std::fs::File::create("index.html").unwrap();
    file.write_all(
        r#"<!DOCTYPE html>
        <html>
        
        <head>
          <!-- 当打开页面的时候使用get请求获取所有消息 -->
          <script>
            // 获取元素
            var displayBox = document.getElementById('displayBox');
            // 发送get请求
            var xhr = new XMLHttpRequest();
            xhr.open('get', '/messages');
            xhr.onreadystatechange = function () {
              if (xhr.readyState === 4 && xhr.status === 200) {
                // 将响应的消息转换为json对象
                var messages = JSON.parse(xhr.responseText);
                // 遍历消息数组
                messages.forEach(function (message) {
                  // 创建一个新的段落元素
                  var p = document.createElement('p');
                  // 设置段落元素的文本内容
                  p.innerText = message.message + "\n" + message.create_time;
                  // 将段落元素添加到消息显示框中
                  displayBox.appendChild(p);
                });
              }
            };
            xhr.send();
          </script>
        
        </head>
        
        <body>
          <div class="container">
            <!-- 第一行：一个消息 -->
            <p id="message">请在下方输入要说的话，并点击发送</p>
            <!-- 第二行：一个输入框 -->
            <form action="/" method="post">
        
              <input type="text" id="inputBox" />
              <!-- 第三行：一个按钮 -->
              <button id="submitButton">发送</button>
              <!-- 第四行：一个消息显示框 -->
            </form>
            <div id="displayBox"></div>
            <script>
              // 获取元素
              var message = document.getElementById('message');
              var inputBox = document.getElementById('inputBox');
              var submitButton = document.getElementById('submitButton');
              var displayBox = document.getElementById('displayBox');
              submitButton.onclick = function () {
                var content = inputBox.value;
                var data = "message=" + content;
        
                var xhr = new XMLHttpRequest();
                xhr.withCredentials = true;
        
                xhr.addEventListener("readystatechange", function () {
                  if (this.readyState === 4) {
                    console.log(this.responseText);
                  }
                });
        
                xhr.open("POST", "/send");
                xhr.setRequestHeader("Content-Type", "application/x-www-form-urlencoded");
                xhr.send(data);

                displayBox.innerHTML = "";
                var p = document.createElement('p');
                // 设置段落元素的文本内容
                var xhr = new XMLHttpRequest();
            xhr.open('get', '/messages');
            xhr.onreadystatechange = function () {
              if (xhr.readyState === 4 && xhr.status === 200) {
                // 将响应的消息转换为json对象
                var messages = JSON.parse(xhr.responseText);
                // 遍历消息数组
                messages.forEach(function (message) {
                  // 创建一个新的段落元素
                  var p = document.createElement('p');
                  // 设置段落元素的文本内容
                  p.innerText = message.message + "\n" + message.create_time;
                  // 将段落元素添加到消息显示框中
                  displayBox.appendChild(p);
                });
              }
            };
            xhr.send();
              };
            </script>
        
          </div>
        </html>
        "#
        .as_bytes(),
    )?;

    let config_sql_url = String::from("sqlite://sqlite.db");

    if !Sqlite::database_exists(&config_sql_url)
        .await
        .unwrap_or(false)
    {
        match Sqlite::create_database(&config_sql_url).await {
            Ok(_) => println!("Create db success"),
            Err(error) => panic!("error: {}", error),
        }
    }

    let mut conn = getconn(&config_sql_url).await;

    // 创建聊天消息存储表
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS chat_message (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message TEXT NOT NULL,
            create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        "#,
    )
    .execute(&mut conn)
    .await
    .unwrap();

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(config_sql_url.clone()))
            // 发送消息
            .service(web::resource("/").to(chat_route))
            .route("/send", web::post().to(send_message))
            .route("/messages", web::get().to(get_message))
            .service(Files::new("/", "./"))
            .wrap(Logger::default())
    })
    .bind(("127.0.0.1", 2000))?
    .bind(("[::1]", 2000))?
    .run()
    .await
}

#[derive(Debug, Serialize)]
pub struct UserMessage {
    id: i32,
    message: String,
    create_time: String,
}

#[derive(Deserialize)]
pub struct FormData {
    message: String,
}

pub async fn send_message(config: web::Data<String>, form: web::Form<FormData>) -> impl Responder {
    let mut conn = getconn(&config).await;
    sqlx::query(
        r#"
        INSERT INTO chat_message (message) VALUES (?)
        "#,
    )
    .bind(&form.message)
    .execute(&mut conn)
    .await
    .unwrap();
    send_code(form.message.clone()).await
}

pub async fn chat_route(config: web::Data<String>) -> impl Responder {
    let mut conn = getconn(config.get_ref()).await;
    let result = sqlx::query("SELECT * FROM chat_message")
        .fetch_all(&mut conn)
        .await
        .unwrap();
    let mut user_messages: Vec<UserMessage> = Vec::new();
    for sql_data in result {
        let id: i32 = sql_data.get("id");
        let message: String = sql_data.get("message");
        let create_time: String = sql_data.get("create_time");
        user_messages.push(UserMessage {
            id,
            message,
            create_time,
        })
    }

    NamedFile::open_async("./index.html").await.unwrap()
}

pub async fn get_message(config: web::Data<String>) -> impl Responder {
    let mut conn = getconn(config.get_ref()).await;
    // let result = sqlx::query("SELECT * FROM chat_message")
    // 倒序查询
    let result = sqlx::query("SELECT * FROM chat_message ORDER BY id DESC")
        .fetch_all(&mut conn)
        .await
        .unwrap();
    let mut user_messages: Vec<UserMessage> = Vec::new();
    for sql_data in result {
        let id: i32 = sql_data.get("id");
        let message: String = sql_data.get("message");
        let create_time: String = sql_data.get("create_time");
        user_messages.push(UserMessage {
            id,
            message,
            create_time,
        })
    }
    HttpResponse::Ok().json(user_messages)
}

pub async fn send_code(message_data: String) -> HttpResponse {
    let address = "738270846@qq.com".parse::<Address>().unwrap();
    let mailbox = Mailbox::new(Some("紧急联系人".to_owned()), address);

    let to_mail = Message::builder()
        .from(mailbox) //发送者
        .to("738270846@qq.com".to_owned().parse().unwrap()) //接收者
        .subject("紧急消息")
        .header(ContentType::TEXT_HTML)
        .body(message_data) //邮箱内容
        .unwrap();

    let mailer = SmtpTransport::relay("smtp.qq.com")
        .unwrap()
        .credentials(Credentials::new(
            "738270846@qq.com".to_owned(),
            "crmhckfoigatbgaa".to_owned(),
        ))
        .build();

    match mailer.send(&to_mail) {
        Ok(_) => HttpResponse::Ok().json(ResponseMessage {
            r#type: "success",
            message: "发送成功",
        }),
        Err(_err) => HttpResponse::Ok().json(ResponseMessage {
            r#type: "error",
            message: &format!("{:?}", _err),
        }),
    }
}

// 请求时消息
#[derive(Debug, Serialize, Deserialize)]
pub struct ResponseMessage<'a> {
    pub r#type: &'a str,
    pub message: &'a str,
}
