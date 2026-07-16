import canva from "../assets/marketplace/canva.svg";
import fathom from "../assets/marketplace/fathom.png";
import intercom from "../assets/marketplace/intercom.svg";
import linear from "../assets/marketplace/linear.svg";
import neon from "../assets/marketplace/neon.svg";
import notion from "../assets/marketplace/notion.svg";
import sentry from "../assets/marketplace/sentry.svg";
import stripe from "../assets/marketplace/stripe.svg";
import { marketplaceIdForUrl } from "./marketplace.js";

const icons = {
  canva,
  fathom,
  intercom,
  linear,
  neon,
  notion,
  sentry,
  stripe,
};

/**
 * Inputs: catalog entry id. Outputs: bundled icon URL or empty string.
 */
export function marketplaceIcon(id) {
  return icons[id] || "";
}

/**
 * Inputs: MCP URL. Outputs: bundled provider icon URL or empty string.
 */
export function providerIconForUrl(url) {
  return marketplaceIcon(marketplaceIdForUrl(url));
}
