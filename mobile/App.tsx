import { StatusBar } from "expo-status-bar";
import { useEffect, useRef, useState } from "react";
import { SafeAreaView, StyleSheet, Text, View } from "react-native";
import { WebView, WebViewMessageEvent } from "react-native-webview";
import {
  buildWalletConnectUriInjection,
  parseWalletConnectSessionMessage,
  subscribeToWalletConnectDeepLinks,
} from "./src/wallet-connect-bridge";

// WalletConnect Cloud Project ID — required for the web content's
// WalletConnectSigner to initialize. See https://cloud.walletconnect.com.
// Mirrors the SDK's WALLETCONNECT_PROJECT_ID convention, exposed here so the
// bridge can pass it through to the WebView without hardcoding it there.
const WALLETCONNECT_PROJECT_ID =
    typeof process !== "undefined" &&
    process.env &&
    process.env.EXPO_PUBLIC_WALLETCONNECT_PROJECT_ID
        ? process.env.EXPO_PUBLIC_WALLETCONNECT_PROJECT_ID
        : null;

const GAME_URL =
    typeof process !== "undefined" &&
    process.env &&
    process.env.EXPO_PUBLIC_GAME_URL
        ? process.env.EXPO_PUBLIC_GAME_URL
        : "https://stellar.org";

// Contract RPC endpoint — override via EXPO_PUBLIC_RPC_URL for custom nodes.
const RPC_URL =
    typeof process !== "undefined" &&
    process.env &&
    process.env.EXPO_PUBLIC_RPC_URL
        ? process.env.EXPO_PUBLIC_RPC_URL
        : null;

// Injected into the WebView so the game frontend can call the batch mobile
// contract view and the event-subscription method without additional round-trips.
const MOBILE_BRIDGE_SCRIPT = `
(function() {
  if (!window.__stellarMobile) {
    window.__stellarMobile = {
      rpcUrl: ${JSON.stringify(RPC_URL)},
      walletConnectProjectId: ${JSON.stringify(WALLETCONNECT_PROJECT_ID)},

      // Overridden by the web content once its WalletConnectSigner is ready.
      // Native calls this with the raw "wc:..." pairing URI whenever a
      // WalletConnect deep link opens the app.
      onWalletConnectURI: null,

      // Web content calls this once a WalletConnect session is approved or
      // torn down, so the native chrome (and any other native listeners)
      // can reflect connection status.
      notifyWalletConnectSession: function(status, account, publicKey) {
        if (window.ReactNativeWebView && window.ReactNativeWebView.postMessage) {
          window.ReactNativeWebView.postMessage(JSON.stringify({
            type: "wallet_connect_session",
            status: status,
            account: account,
            publicKey: publicKey,
          }));
        }
      },

      // Call batch_get_mobile_info — returns dashboard + scan preview in one RPC call.
      batchGetMobileInfo: async function(playerAddress) {
        if (!this.rpcUrl) return null;
        try {
          const res = await fetch(this.rpcUrl, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              jsonrpc: "2.0", id: 1,
              method: "simulateTransaction",
              params: { transaction: { function: "batch_get_mobile_info", args: [playerAddress] } },
            }),
          });
          return await res.json();
        } catch (e) {
          console.warn("[stellar-mobile] batchGetMobileInfo failed:", e);
          return null;
        }
      },

      // Call subscribe_mobile_events — registers the player for push updates.
      subscribeMobileEvents: async function(playerAddress) {
        if (!this.rpcUrl) return;
        try {
          await fetch(this.rpcUrl, {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              jsonrpc: "2.0", id: 2,
              method: "sendTransaction",
              params: { transaction: { function: "subscribe_mobile_events", args: [playerAddress] } },
            }),
          });
        } catch (e) {
          console.warn("[stellar-mobile] subscribeMobileEvents failed:", e);
        }
      },
    };
  }
  true;
})();
`;

export default function App() {
    const webViewRef = useRef<WebView>(null);
    const [ready, setReady] = useState(false);
    const [walletStatus, setWalletStatus] = useState<
        "disconnected" | "connected"
    >("disconnected");
    const pendingWalletConnectUri = useRef<string | null>(null);

    // Intercept wc:/universal-link deep links at the native app-shell level
    // (tapped from a wallet app, a QR scanner, or a cold start) and relay
    // the pairing URI into the WebView's WalletConnectSigner.
    useEffect(() => {
        const subscription = subscribeToWalletConnectDeepLinks({
            onPairingUri: (uri) => {
                if (ready && webViewRef.current) {
                    webViewRef.current.injectJavaScript(
                        buildWalletConnectUriInjection(uri),
                    );
                } else {
                    // WebView isn't loaded yet (cold start race) — flush once ready.
                    pendingWalletConnectUri.current = uri;
                }
            },
        });
        return () => subscription.remove();
    }, [ready]);

    const handleLoad = () => {
        setReady(true);
        if (pendingWalletConnectUri.current && webViewRef.current) {
            webViewRef.current.injectJavaScript(
                buildWalletConnectUriInjection(pendingWalletConnectUri.current),
            );
            pendingWalletConnectUri.current = null;
        }
    };

    const handleMessage = (event: WebViewMessageEvent) => {
        const message = parseWalletConnectSessionMessage(
            event.nativeEvent.data,
        );
        if (message) {
            setWalletStatus(
                message.status === "connected" ? "connected" : "disconnected",
            );
        }
    };

    return (
        <SafeAreaView style={styles.safe}>
            <StatusBar style="light" />
            <View style={styles.chrome}>
                <Text style={styles.title}>Stellar Nebula Nomad</Text>
                <Text style={styles.sub}>
                    {ready
                        ? "Connected — batch contract views active"
                        : "Loading…"}
                    {"  •  Wallet: "}
                    {walletStatus === "connected" ? "connected" : "not connected"}
                </Text>
            </View>
            <WebView
                ref={webViewRef}
                source={{ uri: GAME_URL }}
                style={styles.web}
                injectedJavaScriptBeforeContentLoaded={MOBILE_BRIDGE_SCRIPT}
                onLoad={handleLoad}
                onMessage={handleMessage}
            />
        </SafeAreaView>
    );
}

const styles = StyleSheet.create({
    safe: { flex: 1, backgroundColor: "#0b1020" },
    chrome: { padding: 12, borderBottomWidth: 1, borderBottomColor: "#273054" },
    title: { color: "#e8eefc", fontSize: 18, fontWeight: "600" },
    sub: { color: "#a8b6da", fontSize: 12, marginTop: 6, lineHeight: 18 },
    web: { flex: 1 },
});
