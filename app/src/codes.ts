// Content of QR codes in the code wizard: structured kinds (WLAN, contact,
// e-mail, phone, link) are built into the usual payload strings and parsed
// back when an existing code is edited.

export type CodeKind = "text" | "url" | "wifi" | "vcard" | "email" | "tel";

export interface CodeFields {
  text: string;
  url: string;
  ssid: string;
  password: string;
  security: "WPA" | "WEP" | "nopass";
  hidden: boolean;
  name: string;
  org: string;
  phone: string;
  email: string;
  web: string;
  subject: string;
}

export const emptyFields = (): CodeFields => ({
  text: "",
  url: "https://",
  ssid: "",
  password: "",
  security: "WPA",
  hidden: false,
  name: "",
  org: "",
  phone: "",
  email: "",
  web: "",
  subject: "",
});

/** Backslash-escapes the characters with meaning in `WIFI:` payloads. */
const wifiEscape = (s: string) => s.replace(/([\\;,:"])/g, "\\$1");
/** Escapes vCard 3.0 text values. */
const vcardEscape = (s: string) => s.replace(/([\\;,])/g, "\\$1").replace(/\n/g, "\\n");

const unescape = (s: string) => s.replace(/\\n/g, "\n").replace(/\\(.)/g, "$1");

/** Splits `s` at unescaped `sep`. */
function splitUnescaped(s: string, sep: string): string[] {
  const out: string[] = [];
  let cur = "";
  for (let i = 0; i < s.length; i++) {
    if (s[i] === "\\" && i + 1 < s.length) {
      cur += s[i] + s[i + 1];
      i++;
    } else if (s[i] === sep) {
      out.push(cur);
      cur = "";
    } else {
      cur += s[i];
    }
  }
  out.push(cur);
  return out;
}

export function buildCode(kind: CodeKind, f: CodeFields): string {
  switch (kind) {
    case "text":
      return f.text;
    case "url":
      return f.url.trim();
    case "wifi": {
      const parts = [`T:${f.security}`, `S:${wifiEscape(f.ssid)}`];
      if (f.security !== "nopass") parts.push(`P:${wifiEscape(f.password)}`);
      if (f.hidden) parts.push("H:true");
      return `WIFI:${parts.join(";")};;`;
    }
    case "vcard": {
      const lines = ["BEGIN:VCARD", "VERSION:3.0", `FN:${vcardEscape(f.name)}`];
      if (f.org) lines.push(`ORG:${vcardEscape(f.org)}`);
      if (f.phone) lines.push(`TEL:${vcardEscape(f.phone)}`);
      if (f.email) lines.push(`EMAIL:${vcardEscape(f.email)}`);
      if (f.web) lines.push(`URL:${vcardEscape(f.web)}`);
      lines.push("END:VCARD");
      return lines.join("\n");
    }
    case "email": {
      const subject = f.subject ? `?subject=${encodeURIComponent(f.subject)}` : "";
      return `mailto:${f.email.trim()}${subject}`;
    }
    case "tel":
      return `tel:${f.phone.replace(/[^\d+*#]/g, "")}`;
  }
}

/** Recognizes the payload kinds [`buildCode`] writes; anything else is text. */
export function parseCode(data: string): { kind: CodeKind; fields: CodeFields } {
  const f = emptyFields();
  if (data.startsWith("WIFI:")) {
    for (const part of splitUnescaped(data.slice(5), ";")) {
      const [key, ...rest] = splitUnescaped(part, ":");
      const value = unescape(rest.join(":"));
      if (key === "T") f.security = value === "WEP" ? "WEP" : value === "nopass" || value === "" ? "nopass" : "WPA";
      else if (key === "S") f.ssid = value;
      else if (key === "P") f.password = value;
      else if (key === "H") f.hidden = value === "true";
    }
    return { kind: "wifi", fields: f };
  }
  if (data.startsWith("BEGIN:VCARD")) {
    for (const line of data.split(/\r?\n/)) {
      const colon = line.indexOf(":");
      if (colon < 0) continue;
      const key = line.slice(0, colon).split(";")[0].toUpperCase();
      const value = unescape(line.slice(colon + 1));
      if (key === "FN") f.name = value;
      else if (key === "ORG") f.org = value;
      else if (key === "TEL") f.phone = value;
      else if (key === "EMAIL") f.email = value;
      else if (key === "URL") f.web = value;
    }
    return { kind: "vcard", fields: f };
  }
  if (data.startsWith("mailto:")) {
    const [addr, query = ""] = data.slice(7).split("?");
    f.email = addr;
    f.subject = new URLSearchParams(query).get("subject") ?? "";
    return { kind: "email", fields: f };
  }
  if (data.startsWith("tel:")) {
    f.phone = data.slice(4);
    return { kind: "tel", fields: f };
  }
  if (/^https?:\/\/\S*$/i.test(data)) {
    f.url = data;
    return { kind: "url", fields: f };
  }
  f.text = data;
  return { kind: "text", fields: f };
}
