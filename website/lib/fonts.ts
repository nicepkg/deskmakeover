import localFont from "next/font/local";

export const satoshi = localFont({
  src: "../app/fonts/Satoshi-Variable.woff2",
  weight: "300 900",
  display: "swap",
  variable: "--font-satoshi",
});
