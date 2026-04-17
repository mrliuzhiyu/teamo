import Section from "../components/Section";
import CloudCtaButton from "../../lib/CloudCtaButton";

/// 云端连接区（未登录占位，M3 登录后补真实状态）
export default function Cloud() {
  return (
    <Section
      title="云端连接"
      description="连接后只有有价值的内容会同步，敏感数据永远本地"
    >
      <CloudCtaButton variant="full" />
    </Section>
  );
}
