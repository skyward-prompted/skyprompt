import "@/styles/globals.css";
import { Metadata } from "next";
import type { AppProps } from "next/app";
// import Layout from '../components/layout';

export const metadata: Metadata = {
  title: "skyprompt",
  description: "AI Screen and Voice Recording Software | skyprompt",
  icons: {
    icon: "/favicon.ico",
  },
  twitter: {
    card: "summary_large_image",
    title: "skyprompt",
    site: "@screen_pipe",
    creator: "@screen_pipe",
    description: "AI Screen and Voice Recording Software | skyprompt",
  },
};

export default function App({ Component, pageProps }: AppProps) {
  return <Component {...pageProps} />;
}
