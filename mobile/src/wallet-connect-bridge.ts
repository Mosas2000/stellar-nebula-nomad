import * as Linking from "expo-linking";

/**
 * Stellar Nebula Nomad is a thin WebView shell around a hosted web build of
 * the game (see App.tsx). The actual WalletConnect v2 SignClient session
 * lives in that web content, using the SDK's WalletConnectSigner.
 *
 * What this module does instead is the one thing that *must* happen at the
 * native app-shell level: when the OS hands this app a `wc:` pairing link
 * (tapped from a wallet app, a QR scanner, or a universal link), intercept
 * it here via expo-linking, then relay the raw pairing URI into the WebView
 * through the existing `window.__stellarMobile` bridge so the web content's
 * SignClient can complete the pairing. Session state flows back out of the
 * WebView the same way every other bridge call does: `onMessage` +
 * `ReactNativeWebView.postMessage`.
 */

/** Extracts a single query-string parameter without relying on a URL polyfill
 *  (RN's global URL/URLSearchParams support varies by version/engine). */
function extractQueryParam(query: string, key: string): string | null {
  const segments = query.split("&");
  for (const segment of segments) {
    if (!segment) continue;
    const eq = segment.indexOf("=");
    const rawKey = eq === -1 ? segment : segment.slice(0, eq);
    let decodedKey: string;
    try {
      decodedKey = decodeURIComponent(rawKey);
    } catch {
      decodedKey = rawKey;
    }
    if (decodedKey === key) {
      const rawValue = eq === -1 ? "" : segment.slice(eq + 1);
      try {
        return decodeURIComponent(rawValue);
      } catch {
        return rawValue;
      }
    }
  }
  return null;
}

/**
 * Parses an incoming deep link URL and returns the WalletConnect pairing
 * URI (`wc:<topic>@2?...`) if it is one, or null otherwise.
 *
 * Handles two shapes:
 *  - A raw pairing URI passed directly as the deep link: `wc:abc123@2?...`
 *  - A wrapped universal/custom-scheme link carrying the pairing URI in a
 *    `uri` query parameter, e.g.
 *    `stellarnebulanomad://wc?uri=wc%3Aabc123%402%3F...`
 *    `https://stellar-nebula-nomad.app/wc?uri=wc%3Aabc123...`
 */
export function parseWalletConnectDeepLink(url: string | null | undefined): string | null {
  if (!url) return null;
  const trimmed = url.trim();
  if (!trimmed) return null;

  if (trimmed.startsWith("wc:")) {
    return trimmed;
  }

  const queryIndex = trimmed.indexOf("?");
  if (queryIndex === -1) return null;

  const query = trimmed.slice(queryIndex + 1);
  const uriParam = extractQueryParam(query, "uri");
  if (uriParam && uriParam.startsWith("wc:")) {
    return uriParam;
  }

  return null;
}

/**
 * Builds the JS to inject into the WebView so its `window.__stellarMobile`
 * bridge (see MOBILE_BRIDGE_SCRIPT in App.tsx) receives the pairing URI.
 * The web content is expected to define `onWalletConnectURI` to hand the
 * URI to its own WalletConnectSigner / SignClient instance.
 */
export function buildWalletConnectUriInjection(uri: string): string {
  const safeUri = JSON.stringify(uri);
  return `
(function() {
  try {
    if (window.__stellarMobile && typeof window.__stellarMobile.onWalletConnectURI === "function") {
      window.__stellarMobile.onWalletConnectURI(${safeUri});
    } else {
      window.__stellarMobilePendingWalletConnectURI = ${safeUri};
    }
  } catch (e) {
    console.warn("[stellar-mobile] failed to relay WalletConnect URI:", e);
  }
  true;
})();
`;
}

export interface WalletConnectSessionMessage {
  type: "wallet_connect_session";
  status: "connected" | "disconnected";
  account?: string;
  publicKey?: string;
}

/** Type guard for the postMessage payload the WebView sends back on session changes. */
export function isWalletConnectSessionMessage(
  value: unknown,
): value is WalletConnectSessionMessage {
  return (
    !!value &&
    typeof value === "object" &&
    (value as { type?: unknown }).type === "wallet_connect_session" &&
    ((value as { status?: unknown }).status === "connected" ||
      (value as { status?: unknown }).status === "disconnected")
  );
}

/** Safely parses a raw WebView onMessage payload (JSON string) into a
 *  WalletConnectSessionMessage, or null if it isn't one. */
export function parseWalletConnectSessionMessage(
  raw: string,
): WalletConnectSessionMessage | null {
  try {
    const parsed = JSON.parse(raw);
    return isWalletConnectSessionMessage(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

export interface WalletConnectDeepLinkHandlers {
  onPairingUri: (uri: string) => void;
}

export interface WalletConnectDeepLinkSubscription {
  remove: () => void;
}

/**
 * Wires up expo-linking so any `wc:`/universal-link deep link that opens
 * this app (cold start via getInitialURL, or a link tapped while the app
 * is already running) is parsed and forwarded to `onPairingUri`.
 */
export function subscribeToWalletConnectDeepLinks(
  handlers: WalletConnectDeepLinkHandlers,
): WalletConnectDeepLinkSubscription {
  const handleUrl = (event: { url: string }) => {
    const pairingUri = parseWalletConnectDeepLink(event.url);
    if (pairingUri) {
      handlers.onPairingUri(pairingUri);
    }
  };

  // App was cold-started from a deep link.
  Linking.getInitialURL()
    .then((url) => {
      if (url) handleUrl({ url });
    })
    .catch((error) => {
      console.warn("[stellar-mobile] getInitialURL failed:", error);
    });

  // App was already running / backgrounded when the link was opened.
  const subscription = Linking.addEventListener("url", handleUrl);
  return { remove: () => subscription.remove() };
}
