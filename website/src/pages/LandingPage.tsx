import Hero from '../components/sections/Hero';
import Features from '../components/sections/Features';
import HowItWorks from '../components/sections/HowItWorks';
import CodeExample from '../components/sections/CodeExample';
import GeneratedOutput from '../components/sections/GeneratedOutput';
import SupportedStacks from '../components/sections/SupportedStacks';
import Installation from '../components/sections/Installation';
import Contributors from '../components/sections/Contributors';
import CTA from '../components/sections/CTA';

export default function LandingPage() {
  return (
    <>
      <Hero />
      <Features />
      <HowItWorks />
      <CodeExample />
      <GeneratedOutput />
      <SupportedStacks />
      <Installation />
      <Contributors />
      <CTA />
    </>
  );
}
