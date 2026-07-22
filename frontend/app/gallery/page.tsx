import type { Metadata } from 'next';
import GalleryClient from './client';

export const metadata: Metadata = {
  title: 'Adopters & Findings Gallery | Sanctifier',
  description: 'Discover real projects using Sanctifier and the critical vulnerabilities it has prevented. Explore our gallery of adopters and featured security findings.',
  openGraph: {
    title: 'Adopters & Findings Gallery | Sanctifier',
    description: 'Discover real projects using Sanctifier and the critical vulnerabilities it has prevented.',
  },
};

export default function GalleryPage() {
  return <GalleryClient />;
}
