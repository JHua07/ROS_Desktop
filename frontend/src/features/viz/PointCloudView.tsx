import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { decodeBase64, type VizFrameEvent, type VizFrameHub } from "../../modules/viz";

export interface CloudEntry {
  topic: string;
  hue: number;
}

interface PointCloudViewProps {
  entries: CloudEntry[];
  hub: VizFrameHub;
}

interface CloudBuffer {
  points: THREE.Points;
  positionAttribute: THREE.BufferAttribute;
  colorAttribute: THREE.BufferAttribute;
  positions: Float32Array;
  colors: Float32Array;
  capacity: number;
  lastSeq: number | null;
  lut: Float32Array;
}

// vlpos/msg/H2oPointCloud payload records: u32 timestamp, i16 x/y/z, u8 intensity, u8 ring, u8 tag.
const RECORD_BYTES = 13;
const LUT_SIZE = 256;
const MIN_CAPACITY = 1024;

// Simple jet-like ramp approximation: t=0 blue, t=0.5 green, t=1 red.
function rampColor(t: number): [number, number, number] {
  const clamp01 = (value: number) => Math.min(1, Math.max(0, value));
  return [
    clamp01(1.5 - Math.abs(4 * t - 3)),
    clamp01(1.5 - Math.abs(4 * t - 2)),
    clamp01(1.5 - Math.abs(4 * t - 1)),
  ];
}

function buildLut(hue: number): Float32Array {
  const lut = new Float32Array(LUT_SIZE * 3);
  const color = new THREE.Color();
  for (let index = 0; index < LUT_SIZE; index += 1) {
    const [red, green, blue] = rampColor(index / (LUT_SIZE - 1));
    color.setRGB(red, green, blue);
    color.offsetHSL(hue, 0, 0);
    lut[index * 3] = color.r;
    lut[index * 3 + 1] = color.g;
    lut[index * 3 + 2] = color.b;
  }
  return lut;
}

function createCloud(scene: THREE.Scene, hue: number): CloudBuffer {
  const geometry = new THREE.BufferGeometry();
  const material = new THREE.PointsMaterial({ vertexColors: true, size: 0.06, sizeAttenuation: true });
  const points = new THREE.Points(geometry, material);
  // Frame updates rewrite the whole buffer; stale bounding spheres must never cull a live cloud.
  points.frustumCulled = false;
  scene.add(points);
  const positions = new Float32Array(0);
  const colors = new Float32Array(0);
  const positionAttribute = new THREE.BufferAttribute(positions, 3);
  const colorAttribute = new THREE.BufferAttribute(colors, 3);
  geometry.setAttribute("position", positionAttribute);
  geometry.setAttribute("color", colorAttribute);
  return { points, positionAttribute, colorAttribute, positions, colors, capacity: 0, lastSeq: null, lut: buildLut(hue) };
}

function growCloud(cloud: CloudBuffer, needed: number): void {
  const capacity = Math.max(needed, cloud.capacity * 2, MIN_CAPACITY);
  cloud.positions = new Float32Array(capacity * 3);
  cloud.colors = new Float32Array(capacity * 3);
  cloud.positionAttribute = new THREE.BufferAttribute(cloud.positions, 3);
  cloud.colorAttribute = new THREE.BufferAttribute(cloud.colors, 3);
  cloud.points.geometry.setAttribute("position", cloud.positionAttribute);
  cloud.points.geometry.setAttribute("color", cloud.colorAttribute);
  cloud.capacity = capacity;
}

function applyFrame(cloud: CloudBuffer, frame: VizFrameEvent): void {
  if (cloud.lastSeq === frame.seq) return;
  cloud.lastSeq = frame.seq;
  const bytes = decodeBase64(frame.data_b64);
  const recordCount = Math.floor(bytes.byteLength / RECORD_BYTES);
  const pointNum = Math.min(frame.meta.point_num ?? recordCount, recordCount);
  if (pointNum <= 0) {
    cloud.points.geometry.setDrawRange(0, 0);
    return;
  }
  if (pointNum > cloud.capacity) growCloud(cloud, pointNum);
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const { positions, colors, lut } = cloud;
  const scale = (frame.meta.scale ?? 0) > 0 ? (frame.meta.scale as number) : 1;
  for (let index = 0; index < pointNum; index += 1) {
    const offset = index * RECORD_BYTES;
    const base = index * 3;
    positions[base] = view.getInt16(offset + 4, true) * scale;
    positions[base + 1] = view.getInt16(offset + 6, true) * scale;
    positions[base + 2] = view.getInt16(offset + 8, true) * scale;
    const intensity = bytes[offset + 10];
    colors[base] = lut[intensity * 3];
    colors[base + 1] = lut[intensity * 3 + 1];
    colors[base + 2] = lut[intensity * 3 + 2];
  }
  cloud.points.geometry.setDrawRange(0, pointNum);
  cloud.positionAttribute.needsUpdate = true;
  cloud.colorAttribute.needsUpdate = true;
}

export function PointCloudView({ entries, hub }: PointCloudViewProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cloudsRef = useRef(new Map<string, CloudBuffer>());

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(60, 1, 0.1, 5000);
    camera.position.set(30, 25, 30);
    camera.lookAt(0, 0, 0);
    const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    // Vertex colors are computed directly; skip the sRGB output transform so hues show as authored.
    renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
    renderer.domElement.className = "viz-canvas";
    container.appendChild(renderer.domElement);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.target.set(0, 0, 0);

    const resize = () => {
      const width = container.clientWidth || 1;
      const height = container.clientHeight || 1;
      renderer.setSize(width, height, false);
      camera.aspect = width / height;
      camera.updateProjectionMatrix();
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(container);

    let raf = 0;
    const render = () => {
      raf = requestAnimationFrame(render);
      controls.update();
      renderer.render(scene, camera);
    };
    render();

    sceneRef.current = scene;
    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      controls.dispose();
      for (const cloud of cloudsRef.current.values()) {
        cloud.points.geometry.dispose();
        (cloud.points.material as THREE.Material).dispose();
      }
      cloudsRef.current.clear();
      renderer.dispose();
      renderer.domElement.remove();
      sceneRef.current = null;
    };
  }, []);

  useEffect(() => {
    const scene = sceneRef.current;
    if (!scene) return;
    const desired = new Set(entries.map((entry) => entry.topic));
    for (const [topic, cloud] of [...cloudsRef.current]) {
      if (!desired.has(topic)) {
        scene.remove(cloud.points);
        cloud.points.geometry.dispose();
        (cloud.points.material as THREE.Material).dispose();
        cloudsRef.current.delete(topic);
      }
    }
    for (const entry of entries) {
      if (!cloudsRef.current.has(entry.topic)) {
        cloudsRef.current.set(entry.topic, createCloud(scene, entry.hue));
      }
    }
    const unsubscribes = entries.map((entry) =>
      hub.subscribe(entry.topic, (frame) => {
        const cloud = cloudsRef.current.get(entry.topic);
        if (cloud) applyFrame(cloud, frame);
      }),
    );
    return () => {
      for (const unsubscribe of unsubscribes) unsubscribe();
    };
  }, [entries, hub]);

  return <div className="viz-viewport" ref={containerRef} />;
}

export default PointCloudView;
