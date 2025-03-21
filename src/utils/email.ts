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
			from: emailConfig.user,
			to,
			subject: "友链提交验证码",
			html: `
				<div style="font-family: Arial, sans-serif; max-width: 600px; margin: 0 auto;">
					<h2>验证码</h2>
					<p>您的验证码是：<strong style="font-size: 20px; color: #4a90e2;">${code}</strong></p>
					<p>此验证码将在10分钟后过期。</p>
					<p>如果这不是您的操作，请忽略此邮件。</p>
				</div>
			`,
		});
		return true;
	} catch (error) {
		console.error("发送验证码邮件失败:", error);
		return false;
	}
}
