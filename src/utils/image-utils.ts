import sharp from "sharp";

const formatPriority = [
	{ mimeType: "image/avif", extension: "avif" },
	{ mimeType: "image/webp", extension: "webp" },
	{ mimeType: "image/jpeg", extension: "jpg" },
];
async function convertImage(blob: Buffer, acceptHeader: string): Promise<Buffer> {
	let selectedFormat: string | null = null;

	const acceptTypes = acceptHeader.toLowerCase().split(",");

	for (const format of formatPriority) {
		if (acceptTypes.includes(format.mimeType)) {
			selectedFormat = format.mimeType;
			break;
		}
	}

	if (!selectedFormat) {
		selectedFormat = "image/jpeg";
	}

	const image = sharp(blob);

	switch (selectedFormat) {
		case "image/avif":
			return image.avif().toBuffer();
		case "image/webp":
			return image.webp().toBuffer();
		default:
			return image.jpeg().toBuffer();
	}
}

export async function handleImageRequest(blob: Blob, acceptHeader: string): Promise<{ body: Buffer; headers: { [key: string]: string } }> {
	try {
		const buffer = Buffer.from(await blob.arrayBuffer());

		const convertedImage = await convertImage(buffer, acceptHeader);

		const contentType = acceptHeader.includes("avif") ? "image/avif" : acceptHeader.includes("webp") ? "image/webp" : "image/jpeg";

		return {
			body: convertedImage,
			headers: {
				"Content-Type": contentType,
			},
		};
	} catch (error) {
		console.error("Error processing image:", error);

		return {
			body: Buffer.from("Failed to process image"),
			headers: {
				"Content-Type": "text/plain",
			},
		};
	}
}