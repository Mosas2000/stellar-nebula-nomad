import {
  generatePairingQrCodeDataUrl,
  generatePairingQrCodeSvg,
} from "./qrcode";

const SAMPLE_URI =
  "wc:7f6e504bfad60b485450578e05678ed3e8e8c47bd1e0e024f5e5b9c2a7c5b1c@2?relay-protocol=irn&symKey=587d9municated";

describe("generatePairingQrCodeDataUrl", () => {
  it("renders a WalletConnect pairing URI as a PNG data URI", async () => {
    const dataUrl = await generatePairingQrCodeDataUrl(SAMPLE_URI);
    expect(dataUrl).toMatch(/^data:image\/png;base64,/);
    expect(dataUrl.length).toBeGreaterThan(100);
  });

  it("is deterministic for the same input", async () => {
    const a = await generatePairingQrCodeDataUrl(SAMPLE_URI);
    const b = await generatePairingQrCodeDataUrl(SAMPLE_URI);
    expect(a).toBe(b);
  });

  it("rejects an empty URI", async () => {
    await expect(generatePairingQrCodeDataUrl("")).rejects.toThrow(
      /Cannot generate a QR code for an empty URI/,
    );
  });
});

describe("generatePairingQrCodeSvg", () => {
  it("renders a WalletConnect pairing URI as inline SVG", async () => {
    const svg = await generatePairingQrCodeSvg(SAMPLE_URI);
    expect(svg).toContain("<svg");
    expect(svg).toContain("</svg>");
  });

  it("rejects an empty URI", async () => {
    await expect(generatePairingQrCodeSvg("")).rejects.toThrow(
      /Cannot generate a QR code for an empty URI/,
    );
  });
});
