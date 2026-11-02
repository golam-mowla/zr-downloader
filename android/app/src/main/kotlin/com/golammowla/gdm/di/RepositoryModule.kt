package com.golammowla.gdm.di

import com.golammowla.gdm.data.repository.DownloadRepositoryImpl
import com.golammowla.gdm.domain.repository.DownloadRepository
import dagger.Binds
import dagger.Module
import dagger.hilt.InstallIn
import dagger.hilt.components.SingletonComponent
import javax.inject.Singleton

@Module
@InstallIn(SingletonComponent::class)
abstract class RepositoryModule {

    @Binds
    @Singleton
    abstract fun bindDownloadRepository(
        impl: DownloadRepositoryImpl
    ): DownloadRepository
}
