export default function BorderOverlay() {
  return (
    <div className="fixed inset-0 pointer-events-none overflow-hidden border-overlay-enter">
      {/* Subtle 1px border frame */}
      <div className="border-frame" />

      {/* Soft edge glows - blue gradient fading inward */}
      <div className="edge-glow edge-top" />
      <div className="edge-glow edge-bottom" />
      <div className="edge-glow edge-left" />
      <div className="edge-glow edge-right" />

      {/* Traveling light beams - energy racing around the border */}
      <div className="beam-h beam-h-top" />
      <div className="beam-h beam-h-bottom" />
      <div className="beam-v beam-v-left" />
      <div className="beam-v beam-v-right" />

      {/* Corner accent glows - bright pulses at corners */}
      <div className="corner-glow corner-tl" />
      <div className="corner-glow corner-tr" />
      <div className="corner-glow corner-bl" />
      <div className="corner-glow corner-br" />
    </div>
  );
}
