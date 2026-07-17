import { Landing } from "@/components/landing";
import { getDict } from "@/content";

export default function HomePage() {
  return <Landing dict={getDict("en")} />;
}
