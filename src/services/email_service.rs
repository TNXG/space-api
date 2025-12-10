use crate::config::settings::EmailConfig;
use crate::{Error, Result};
use lettre::{
    message::header::ContentType, transport::smtp::authentication::Credentials, AsyncSmtpTransport,
    AsyncTransport, Message, Tokio1Executor,
};

pub struct EmailService {
    config: EmailConfig,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl EmailService {
    pub fn new(config: EmailConfig) -> Result<Self> {
        let creds = Credentials::new(config.username.clone(), config.password.clone());

        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_server)
            .map_err(|e| Error::Internal(format!("Failed to create SMTP transport: {}", e)))?
            .credentials(creds)
            .port(config.smtp_port)
            .build();

        Ok(Self { config, transport })
    }

    pub async fn send_email(
        &self,
        to: &str,
        subject: &str,
        text_body: &str,
        html_body: Option<&str>,
    ) -> Result<()> {
        // 创建邮件
        // 构建发件人显示名，如果配置里有完整的 display 格式则直接使用，否则按 "名字 <邮箱>" 格式构建
        let from_header = if self.config.from_address.contains('<') || self.config.from_address.contains('>') {
            self.config.from_address.clone()
        } else {
            format!("{} <{}>", self.config.from_name, self.config.from_address)
        };

        let message_builder = Message::builder()
            .from(
                from_header
                    .parse()
                    .map_err(|e| Error::Internal(format!("Invalid from address: {}", e)))?,
            )
            .to(to
                .parse()
                .map_err(|e| Error::Internal(format!("Invalid to address: {}", e)))?)
            .subject(subject);

        // 添加内容
        let message = if let Some(html) = html_body {
            message_builder
                .header(ContentType::TEXT_HTML)
                .body(html.to_string())
                .map_err(|e| Error::Internal(format!("Failed to build message: {}", e)))?
        } else {
            message_builder
                .header(ContentType::TEXT_PLAIN)
                .body(text_body.to_string())
                .map_err(|e| Error::Internal(format!("Failed to build message: {}", e)))?
        };

        // 发送邮件
        self.transport
            .send(message)
            .await
            .map_err(|e| Error::Internal(format!("Failed to send email: {}", e)))?;

        Ok(())
    }

    // 假设这是在你的 impl 块中
    pub async fn send_verification_email(&self, to: &str, verification_code: &str) -> Result<()> {
        // 将验证码包含在邮件主题中，方便用户在邮箱列表里直接识别
        let subject = format!("【天翔TNXG】邮箱验证码：{}", verification_code);

        // 纯文本回退版本（保持简洁）
        let text_body = format!(
        "您好，\n\n您的验证码是: {}\n\n此验证码将在10分钟内有效。请勿泄露给他人。\n\n天翔TNXGの空间站",
        verification_code
    );

        // HTML 版本
        // 注意：在 Rust format! 宏中，CSS 的花括号 { } 需要被转义为 {{ }}
        // {verification_code} 是我们要替换的变量
        let html_body = format!(
            r#"
<!DOCTYPE html>
<html lang="zh-CN">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <title>{subject}</title>
    <style>
        /* 重置样式 */
        body, table, td, a {{ -webkit-text-size-adjust: 100%; -ms-text-size-adjust: 100%; }}
        table, td {{ mso-table-lspace: 0pt; mso-table-rspace: 0pt; }}
        img {{ -ms-interpolation-mode: bicubic; }}
        
        /* 基础字体 - 优先使用系统无衬线字体 */
        body {{
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", "Microsoft YaHei", "Noto Sans SC", Arial, sans-serif;
            margin: 0;
            padding: 0;
            width: 100% !important;
        }}

        /* 深色模式适配 */
        @media (prefers-color-scheme: dark) {{
            .body-bg {{ background-color: #1a1a1a !important; }}
            .content-card {{ background-color: #2d2d2d !important; border-color: #444444 !important; }}
            .text-primary {{ color: #e0e0e0 !important; }}
            .text-secondary {{ color: #a0a0a0 !important; }}
            .code-box {{ background-color: #3d3d3d !important; color: #ff6b6b !important; border-color: #555555 !important; }}
            .footer-text {{ color: #666666 !important; }}
        }}
    </style>
</head>
<body class="body-bg" style="margin: 0; padding: 0; background-color: #f7f7f5; -webkit-font-smoothing: antialiased;">
    <table role="presentation" border="0" cellpadding="0" cellspacing="0" width="100%" class="body-bg" style="background-color: #f7f7f5;">
        <tr>
            <td align="center" style="padding: 40px 10px;">
                <table role="presentation" border="0" cellpadding="0" cellspacing="0" width="100%" style="max-width: 600px;">
                    <tr>
                        <td class="content-card" style="background-color: #ffffff; padding: 40px; border-radius: 8px; box-shadow: 0 4px 15px rgba(0,0,0,0.05); border-top: 4px solid #8E2E21; text-align: left;">
                            <h1 class="text-primary" style="margin: 0 0 20px 0; font-family: 'Songti SC', 'SimSun', serif; font-size: 24px; font-weight: bold; color: #333333; letter-spacing: 1px;">
                                邮箱验证
                            </h1>
                            <p class="text-primary" style="margin: 0 0 15px 0; font-size: 16px; line-height: 1.6; color: #333333;">
                                尊敬的探索者，您好：
                            </p>
                            <p class="text-secondary" style="margin: 0 0 25px 0; font-size: 15px; line-height: 1.6; color: #555555;">
                                欢迎来到 <strong>天翔TNXGの空间站</strong>。您正在进行身份验证，请使用下方的验证码完成操作。
                            </p>
                            <div class="code-box" style="background-color: #f9f9f9; border: 1px dashed #cccccc; border-radius: 4px; padding: 20px; text-align: center; margin: 30px 0;">
                                <span style="font-family: 'Courier New', monospace; font-size: 32px; font-weight: bold; letter-spacing: 8px; color: #8E2E21; display: block;">
                                {verification_code}
                                </span>
                            </div>
                            <p class="text-secondary" style="margin: 0 0 10px 0; font-size: 14px; line-height: 1.6; color: #666666;">
                                * 此验证码将在 <strong>10分钟</strong> 内有效。
                            </p>
                            <p class="text-secondary" style="margin: 0 0 30px 0; font-size: 14px; line-height: 1.6; color: #666666;">
                                * 如果这不是您的操作，请忽略此邮件。
                            </p>
                            <div style="border-top: 1px solid #eeeeee; margin: 30px 0;"></div>
                            <div style="text-align: right;">
                                <p class="text-primary" style="margin: 0; font-family: 'Songti SC', 'SimSun', serif; font-size: 16px; font-weight: bold; color: #333333;">
                                    天翔TNXGの空间站
                                </p>
                                <p class="text-secondary" style="margin: 5px 0 0 0; font-size: 12px; color: #888888;">
                                    私たちはもう、舞台の上。
                                </p>
                            </div>
                            
                        </td>
                    </tr>
                    <tr>
                        <td align="center" style="padding-top: 20px;">
                            <p class="footer-text" style="margin: 0; font-size: 12px; color: #999999; line-height: 1.5;">
                                © {year} 天翔TNXG. All rights reserved.<br>
                                本邮件由系统自动发送，请勿直接回复。
                            </p>
                        </td>
                    </tr>
                </table>
            </td>
        </tr>
    </table>
</body>
</html>
"#,
            verification_code = verification_code,
            year = chrono::Local::now().format("%Y"), // 假设你用了 chrono 库，如果没有可以写死或者去掉
            subject = subject
        );

        self.send_email(to, &subject, &text_body, Some(&html_body))
            .await
    }
}
