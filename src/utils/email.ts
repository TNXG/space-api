import process from "node:process";
import dotenv from "dotenv";
import nodemailer from "nodemailer";

dotenv.config();

interface EmailConfig {
	host: string;
	port: number;
	user: string;
	pass: string;
}

const emailConfig: EmailConfig = {
	host: process.env.EMAIL_HOST || "",
	port: Number.parseInt(process.env.EMAIL_PORT || "587", 10),
	user: process.env.EMAIL_USER || "",
	pass: process.env.EMAIL_PASS || "",
};

const transporter = nodemailer.createTransport({
	host: emailConfig.host,
	port: emailConfig.port,
	secure: emailConfig.port === 465,
	auth: {
		user: emailConfig.user,
		pass: emailConfig.pass,
	},
});

export async function sendVerificationCode(to: string, code: string): Promise<boolean> {
	try {
		await transporter.sendMail({
			from: `"Rikki Sender!" <${emailConfig.user}>`,
			to,
			subject: "友链提交验证码",
			html: `
				<div style="font-family: Arial, sans-serif; max-width: 600px; margin: 20px auto; padding: 30px; border-radius: 8px; box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1); background-color: #ffffff;">
					<div style="text-align: center; margin-bottom: 30px;">
						<h1 style="color: #333333; margin: 0;">验证码确认</h1>
					</div>
					<div style="margin-bottom: 30px;">
						<p style="color: #666666; font-size: 16px; margin-bottom: 20px;">您好！</p>
						<p style="color: #666666; font-size: 16px; margin-bottom: 20px;">感谢您申请友链提交。请使用以下验证码完成验证：</p>
						<div style="background-color: #f5f5f5; padding: 20px; border-radius: 6px; text-align: center; margin: 20px 0;">
							<span style="font-size: 32px; font-weight: bold; color: #4a90e2; letter-spacing: 5px;">${code}</span>
						</div>
						<p style="color: #666666; font-size: 14px;">此验证码将在<span style="color: #ff4444;">10分钟</span>后过期。</p>
					</div>
					<div style="border-top: 1px solid #eeeeee; padding-top: 20px; text-align: center;">
						<p style="color: #999999; font-size: 14px; margin: 0;">如果这不是您的操作，请忽略此邮件。</p>
						<p style="color: #999999; font-size: 12px; margin-top: 10px;">此邮件由系统自动发送，请勿回复。</p>
					</div>
				</div>
			`,
		});
		return true;
	} catch (error) {
		console.error("发送验证码邮件失败:", error);
		return false;
	}
}
