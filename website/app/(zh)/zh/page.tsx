import { Landing } from "@/components/landing";
import { getDict } from "@/content";

export default function ZhHomePage() {
  return <Landing dict={getDict("zh")} />;
}
