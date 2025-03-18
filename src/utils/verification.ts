import { db_delete, db_find, db_insert } from "./db";

interface VerificationCode {
	email: string;
	code: string;
	createdAt: string;
	expiredAt: string;
	isUsed: boolean;
}

// 生成6位数字验证码
function generateCode(): string {
	return Math.floor(100000 + Math.random() * 900000).toString();
}

// 创建新的验证码
export async function createVerificationCode(email: string): Promise<string | null> {
	try {
		// 删除该邮箱的旧验证码
		await db_delete("space-api", "verification_codes", { email });

		const code = generateCode();
		const now = new Date();
		const expiredAt = new Date(now.getTime() + 10 * 60 * 1000); // 10分钟后过期

		const verificationCode: VerificationCode = {
			email,
			code,
			createdAt: now.toISOString(),
			expiredAt: expiredAt.toISOString(),
			isUsed: false,
		};

		const success = await db_insert("space-api", "verification_codes", verificationCode);
		return success ? code : null;
	}
	catch (error) {
		console.error("创建验证码失败:", error);
		return null;
	}
}

// 验证验证码
export async function verifyCode(email: string, code: string): Promise<boolean> {
	try {
		const verificationCode = await db_find("space-api", "verification_codes", { email, code });

		if (!verificationCode) {
			return false;
		}

		if (verificationCode.isUsed) {
			return false;
		}

		const now = new Date();
		const expiredAt = new Date(verificationCode.expiredAt);

		if (now > expiredAt) {
			return false;
		}

		// 标记验证码为已使用
		await db_delete("space-api", "verification_codes", { email });
		return true;
	}
	catch (error) {
		console.error("验证码验证失败:", error);
		return false;
	}
}
