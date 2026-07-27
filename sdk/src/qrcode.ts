import QRCode from "qrcode";

export interface QrCodeOptions {
  /** Quiet-zone size in modules. Defaults to 1. */
  margin?: number;
  /** Pixel scale per module (PNG only). Defaults to 6. */
  scale?: number;
  /** Force an exact pixel width (PNG only); overrides `scale` if set. */
  width?: number;
  color?: { dark?: string; light?: string };
}

/**
 * Renders a WalletConnect pairing URI (or any string) as a PNG data URI,
 * suitable for an `<img src="...">` in a web or desktop SDK consumer.
 */
export async function generatePairingQrCodeDataUrl(
  uri: string,
  options: QrCodeOptions = {},
): Promise<string> {
  if (!uri) {
    throw new Error("Cannot generate a QR code for an empty URI.");
  }
  return QRCode.toDataURL(uri, {
    margin: options.margin ?? 1,
    scale: options.scale ?? 6,
    width: options.width,
    color: options.color,
  });
}

/**
 * Renders a WalletConnect pairing URI as inline SVG markup, suitable for
 * embedding directly in a page without a data-URI round trip.
 */
export async function generatePairingQrCodeSvg(
  uri: string,
  options: QrCodeOptions = {},
): Promise<string> {
  if (!uri) {
    throw new Error("Cannot generate a QR code for an empty URI.");
  }
  return QRCode.toString(uri, {
    type: "svg",
    margin: options.margin ?? 1,
    color: options.color,
  });
}
