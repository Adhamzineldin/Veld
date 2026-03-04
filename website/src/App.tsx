import Navbar from './components/Navbar';
import Footer from './components/Footer';
import Hero from './components/sections/Hero';
import Features from './components/sections/Features';
import HowItWorks from './components/sections/HowItWorks';
import CodeExample from './components/sections/CodeExample';
import LiveDemo from './components/sections/LiveDemo';
import SupportedStacks from './components/sections/SupportedStacks';
import Installation from './components/sections/Installation';
import Contributors from './components/sections/Contributors';
import CTA from './components/sections/CTA';

export default function App() {
  return (
    <>
      <Navbar />
      <main>
        <Hero />
        <Features />
        <HowItWorks />
        <CodeExample />
        <LiveDemo />
        <SupportedStacks />
        <Installation />
        <Contributors />
        <CTA />
      </main>
      <Footer />
    </>
  );
}
