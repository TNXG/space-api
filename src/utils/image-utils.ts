import { Buffer } from "node:buffer";
import sharp from "sharp";

const cachedImageConversion = defineCachedFunction(
	async (buffer: Buffer, format: string) => {
		const image = sharp(buffer);

		let convertedBuffer: Buffer<ArrayBufferLike> | PromiseLike<Buffer<ArrayBufferLike>>;
		switch (format) {
			case "image/avif":
				convertedBuffer = await image.avif().toBuffer();
				break;
			case "image/webp":
				convertedBuffer = await image.webp().toBuffer();
				break;
			default:
				convertedBuffer = await image.jpeg().toBuffer();
		}

		return convertedBuffer;
	},
	{
		maxAge: 60 * 60,
		name: "imageCache",
	},
);

export const formatAccept = (acceptHeader: string) => {
	const formatPriority = [
		{ mimeType: "image/avif", extension: "avif" },
		{ mimeType: "image/webp", extension: "webp" },
		{ mimeType: "image/jpeg", extension: "jpg" },
	];

	const acceptTypes = acceptHeader.toLowerCase().split(",");

	for (const format of formatPriority) {
		if (acceptTypes.includes(format.mimeType)) {
			return format.mimeType;
		}
	}

	return "image/jpeg";
};

export async function handleImageRequest(blob: Blob, acceptHeader: string): Promise<{ body: Buffer; headers: { [key: string]: string } }> {
	try {
		const buffer = Buffer.from(await blob.arrayBuffer());

		const contentType = formatAccept(acceptHeader);

		const convertedImage = await cachedImageConversion(buffer, contentType);

		return {
			body: Buffer.from(convertedImage),
			headers: {
				"Content-Type": contentType,
			},
		};
	}
	catch (error) {
		console.error("Error processing image:", error);

		const errorResponse: ApiResponse = {
			code: "500",
			message: "Failed to process image",
			status: "error",
		};

		return {
			body: Buffer.from(JSON.stringify(errorResponse)),
			headers: {
				"Content-Type": "application/json",
			},
		};
	}
}
